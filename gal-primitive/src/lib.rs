pub use num_bigint::BigInt;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RawValue {
    Unit,
    Bool(bool),
    Num(BigInt),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueType {
    Unit,
    Bool,
    Num,
    Str,
}

impl Default for RawValue {
    fn default() -> Self {
        Self::Unit
    }
}

impl RawValue {
    pub fn get_type(&self) -> ValueType {
        match self {
            Self::Unit => ValueType::Unit,
            Self::Bool(_) => ValueType::Bool,
            Self::Num(_) => ValueType::Num,
            Self::Str(_) => ValueType::Str,
        }
    }

    pub fn get_bool(&self) -> bool {
        match self {
            Self::Unit => false,
            Self::Bool(b) => *b,
            Self::Num(i) => *i != 0.into(),
            Self::Str(s) => !s.is_empty(),
        }
    }

    pub fn get_num(&self) -> Cow<BigInt> {
        match self {
            Self::Unit => Cow::Owned(0.into()),
            Self::Bool(b) => Cow::Owned((*b as i32).into()),
            Self::Num(i) => Cow::Borrowed(i),
            Self::Str(s) => Cow::Owned(s.len().into()),
        }
    }

    pub fn get_str(&self) -> Cow<str> {
        match self {
            Self::Unit => Cow::default(),
            Self::Bool(b) => b.to_string().into(),
            Self::Num(i) => i.to_string().into(),
            Self::Str(s) => s.as_str().into(),
        }
    }
}

impl<'de> Deserialize<'de> for RawValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = RawValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a unit, boolean, integer, string value")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(RawValue::Unit)
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(RawValue::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(RawValue::Num(v.into()))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(RawValue::Num(v.into()))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(RawValue::Str(v.into()))
            }
        }
        deserializer.deserialize_any(ValueVisitor)
    }
}

impl Serialize for RawValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Unit => serializer.serialize_unit(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Num(n) => n.serialize(serializer),
            Self::Str(s) => serializer.serialize_str(s),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn serde_value() {
        assert_eq!(
            serde_yaml::from_str::<RawValue>("~").unwrap(),
            RawValue::Unit
        );

        assert_eq!(
            serde_yaml::from_str::<RawValue>("true").unwrap(),
            RawValue::Bool(true)
        );
        assert_eq!(
            serde_yaml::from_str::<RawValue>("false").unwrap(),
            RawValue::Bool(false)
        );

        assert_eq!(
            serde_yaml::from_str::<RawValue>("114514").unwrap(),
            RawValue::Num(114514)
        );
        assert_eq!(
            serde_yaml::from_str::<RawValue>("-1919810").unwrap(),
            RawValue::Num(-1919810)
        );

        assert_eq!(
            serde_yaml::from_str::<RawValue>("\"Hello world!\"").unwrap(),
            RawValue::Str("Hello world!".into())
        );

        assert_eq!(serde_yaml::to_string(&RawValue::Unit).unwrap(), "---\n~\n");

        assert_eq!(
            serde_yaml::to_string(&RawValue::Bool(true)).unwrap(),
            "---\ntrue\n"
        );
        assert_eq!(
            serde_yaml::to_string(&RawValue::Bool(false)).unwrap(),
            "---\nfalse\n"
        );

        assert_eq!(
            serde_yaml::to_string(&RawValue::Num(114514)).unwrap(),
            "---\n114514\n"
        );
        assert_eq!(
            serde_yaml::to_string(&RawValue::Num(-1919)).unwrap(),
            "---\n-1919\n"
        );

        assert_eq!(
            serde_yaml::to_string(&RawValue::Str("aaa".into())).unwrap(),
            "---\naaa\n"
        );
    }
}
