//! The script interpreter.

use crate::{plugin::Runtime, *};
use async_trait::async_trait;
use fallback::Fallback;
use gal_script::*;
use log::{error, warn};

/// The variable table in scripts.
pub struct VarTable<'a> {
    /// The plugin runtime.
    pub runtime: &'a Runtime,
    /// The resource map.
    pub res: Fallback<&'a VarMap>,
    /// The context variables.
    pub locals: &'a mut VarMap,
    /// The locale variables.
    pub vars: VarMap,
}

impl<'a> VarTable<'a> {
    /// Creates a new [`VarTable`].
    pub fn new(runtime: &'a Runtime, res: Fallback<&'a VarMap>, locals: &'a mut VarMap) -> Self {
        Self {
            runtime,
            res,
            locals,
            vars: VarMap::default(),
        }
    }

    /// Calls a [`Callable`] object.
    pub async fn call(&mut self, c: &impl Callable) -> RawValue {
        c.call(self).await
    }
}

/// Represents a callable part of a script.
#[async_trait]
pub trait Callable {
    /// Calls the part with the [`VarTable`].
    async fn call(&self, ctx: &mut VarTable) -> RawValue;
}

#[async_trait]
impl<T: Callable + Sync> Callable for &T {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        (*self).call(ctx).await
    }
}

#[async_trait]
impl<T: Callable + Sync> Callable for Option<T> {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        match self {
            Some(c) => c.call(ctx).await,
            None => RawValue::Unit,
        }
    }
}

#[async_trait]
impl Callable for Program {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        ctx.vars.clear();
        let mut res = RawValue::Unit;
        for expr in &self.0 {
            res = expr.call(ctx).await;
        }
        res
    }
}

#[async_trait]
impl Callable for Expr {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        match self {
            Self::Ref(r) => r.call(ctx).await,
            Self::Const(c) => c.clone(),
            Self::Unary(op, e) => match op {
                UnaryOp::Positive => RawValue::Num(e.call(ctx).await.get_num()),
                UnaryOp::Negative => RawValue::Num(-e.call(ctx).await.get_num()),
                UnaryOp::Not => match e.call(ctx).await {
                    RawValue::Unit => RawValue::Unit,
                    RawValue::Bool(b) => RawValue::Bool(!b),
                    RawValue::Num(i) => RawValue::Num(!i),
                    RawValue::Str(_) => RawValue::Str(String::new()),
                },
            },
            Self::Binary(lhs, op, rhs) => match op {
                BinaryOp::Val(op) => bin_val(ctx, lhs, op, rhs).await,
                BinaryOp::Logic(op) => bin_logic(ctx, lhs, op, rhs).await,
                BinaryOp::Assign => {
                    let val = rhs.call(ctx).await;
                    assign(ctx, lhs, val)
                }
                BinaryOp::Inplace(op) => {
                    let val = bin_val(ctx, lhs, op, rhs).await;
                    assign(ctx, lhs, val)
                }
            },
            Self::Call(ns, name, args) => call(ctx, ns, name, args).await,
        }
    }
}

async fn bin_val(ctx: &mut VarTable<'_>, lhs: &Expr, op: &ValBinaryOp, rhs: &Expr) -> RawValue {
    let lhs = lhs.call(ctx).await;
    let rhs = rhs.call(ctx).await;
    let t = lhs.get_type().max(rhs.get_type());
    match t {
        ValueType::Unit => RawValue::Unit,
        ValueType::Bool => bin_bool_val(lhs.get_bool(), op, rhs.get_bool()),
        ValueType::Num => RawValue::Num(bin_num_val(lhs.get_num(), op, rhs.get_num())),
        ValueType::Str => bin_str_val(lhs, op, rhs),
    }
}

fn bin_bool_val(lhs: bool, op: &ValBinaryOp, rhs: bool) -> RawValue {
    match op {
        ValBinaryOp::Add
        | ValBinaryOp::Minus
        | ValBinaryOp::Mul
        | ValBinaryOp::Div
        | ValBinaryOp::Mod => RawValue::Num(bin_num_val(lhs as i64, op, rhs as i64)),
        ValBinaryOp::And => RawValue::Bool(lhs && rhs),
        ValBinaryOp::Or => RawValue::Bool(lhs || rhs),
        ValBinaryOp::Xor => RawValue::Bool(lhs ^ rhs),
    }
}

fn bin_num_val(lhs: i64, op: &ValBinaryOp, rhs: i64) -> i64 {
    match op {
        ValBinaryOp::Add => lhs + rhs,
        ValBinaryOp::Minus => lhs - rhs,
        ValBinaryOp::Mul => lhs * rhs,
        ValBinaryOp::Div => lhs / rhs,
        ValBinaryOp::Mod => lhs % rhs,
        ValBinaryOp::And => lhs & rhs,
        ValBinaryOp::Or => lhs | rhs,
        ValBinaryOp::Xor => lhs ^ rhs,
    }
}

fn bin_str_val(lhs: RawValue, op: &ValBinaryOp, rhs: RawValue) -> RawValue {
    match op {
        ValBinaryOp::Add => RawValue::Str((lhs.get_str() + rhs.get_str()).into()),
        ValBinaryOp::Mul => match (
            lhs.get_type().max(ValueType::Num),
            rhs.get_type().max(ValueType::Num),
        ) {
            (ValueType::Str, ValueType::Str) => unimplemented!(),
            (ValueType::Num, ValueType::Str) => {
                RawValue::Str(rhs.get_str().repeat(lhs.get_num() as usize))
            }
            (ValueType::Str, ValueType::Num) => {
                RawValue::Str(lhs.get_str().repeat(rhs.get_num() as usize))
            }
            _ => unreachable!(),
        },
        _ => unimplemented!(),
    }
}

async fn bin_logic(ctx: &mut VarTable<'_>, lhs: &Expr, op: &LogicBinaryOp, rhs: &Expr) -> RawValue {
    let res = match op {
        LogicBinaryOp::And => lhs.call(ctx).await.get_bool() && rhs.call(ctx).await.get_bool(),
        LogicBinaryOp::Or => lhs.call(ctx).await.get_bool() || rhs.call(ctx).await.get_bool(),
        op => {
            let lhs = lhs.call(ctx).await;
            let rhs = rhs.call(ctx).await;
            let t = lhs.get_type().max(rhs.get_type());
            match t {
                ValueType::Unit => false,
                ValueType::Bool => bin_ord_logic(&lhs.get_bool(), op, &rhs.get_bool()),
                ValueType::Num => bin_ord_logic(&lhs.get_num(), op, &rhs.get_num()),
                ValueType::Str => bin_ord_logic(&lhs.get_str(), op, &rhs.get_str()),
            }
        }
    };
    RawValue::Bool(res)
}

fn bin_ord_logic<T: Ord>(lhs: &T, op: &LogicBinaryOp, rhs: &T) -> bool {
    match op {
        LogicBinaryOp::Eq => lhs == rhs,
        LogicBinaryOp::Neq => lhs != rhs,
        LogicBinaryOp::Lt => lhs < rhs,
        LogicBinaryOp::Le => lhs <= rhs,
        LogicBinaryOp::Gt => lhs > rhs,
        LogicBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn assign(ctx: &mut VarTable, e: &Expr, val: RawValue) -> RawValue {
    match e {
        Expr::Ref(r) => match r {
            Ref::Var(n) => ctx.vars.insert(n.into(), val),
            Ref::Ctx(n) => ctx.locals.insert(n.into(), val),
            Ref::Res(_) => unimplemented!("Resources"),
        },
        _ => unreachable!(),
    };
    RawValue::Unit
}

async fn call(ctx: &mut VarTable<'_>, ns: &str, name: &str, args: &[Expr]) -> RawValue {
    if ns.is_empty() {
        match name {
            "if" => {
                if args.get(0).call(ctx).await.get_bool() {
                    args.get(1)
                } else {
                    args.get(2)
                }
                .call(ctx)
                .await
            }
            _ => unimplemented!("intrinstics"),
        }
    } else {
        let args = {
            let mut res = vec![];
            for e in args {
                res.push(e.call(ctx).await);
            }
            res
        };
        if let Some(runtime) = ctx.runtime.modules.get(ns) {
            match runtime.dispatch_method(name, &args).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Calling `{}.{}` error: {}", ns, name, e);
                    RawValue::Unit
                }
            }
        } else {
            error!("Cannot find namespace `{}`.", ns);
            RawValue::Unit
        }
    }
}

#[async_trait]
impl Callable for Ref {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        match self {
            Self::Var(n) => ctx.vars.get(n).cloned().unwrap_or_else(|| {
                warn!("Cannot find variable `{}`.", n);
                Default::default()
            }),
            Self::Ctx(n) => ctx.locals.get(n).cloned().unwrap_or_else(|| {
                warn!("Cannot find context variable `{}`.", n);
                Default::default()
            }),
            Self::Res(n) => ctx
                .res
                .as_ref()
                .and_then(|map| map.get(n))
                .cloned()
                .unwrap_or_else(|| {
                    warn!("Cannot find resource `{}`.", n);
                    Default::default()
                }),
        }
    }
}

#[async_trait]
impl Callable for Text {
    async fn call(&self, ctx: &mut VarTable) -> RawValue {
        let mut str = String::new();
        for line in &self.0 {
            match line {
                Line::Str(s) => str.push_str(s),
                Line::Cmd(c) => {
                    if let Command::Exec(p) = c {
                        str.push_str(&p.call(ctx).await.get_str())
                    }
                }
            }
        }
        RawValue::Str(str.trim().to_string())
    }
}

#[cfg(test)]
mod test {
    use crate::{plugin::Runtime, script::*};
    use tokio::sync::OnceCell;

    static RUNTIME: OnceCell<Runtime> = OnceCell::const_new();

    async fn get_runtime() -> &'static Runtime {
        RUNTIME
            .get_or_init(|| async {
                let runtime = Runtime::load(
                    "../../examples/plugins",
                    env!("CARGO_MANIFEST_DIR"),
                    &["format"],
                );
                runtime.await.unwrap()
            })
            .await
    }

    #[tokio::test]
    async fn vars() {
        let mut locals = VarMap::default();
        let mut ctx = VarTable::new(get_runtime().await, Fallback::new(None, None), &mut locals);
        assert_eq!(
            ProgramParser::new()
                .parse(
                    "
                            a = 0;
                            a += 1;
                            a += a;
                            a
                        "
                )
                .ok()
                .call(&mut ctx)
                .await,
            RawValue::Num(2)
        );

        assert_eq!(
            ProgramParser::new().parse("a").ok().call(&mut ctx).await,
            RawValue::Unit
        );

        assert_eq!(
            ProgramParser::new()
                .parse(
                    "
                            $a = 0;
                            $a += 1;
                            $a += a;
                            $a
                        "
                )
                .ok()
                .call(&mut ctx)
                .await,
            RawValue::Num(1)
        );

        assert_eq!(
            ProgramParser::new().parse("$a").ok().call(&mut ctx).await,
            RawValue::Num(1)
        );
    }

    #[tokio::test]
    async fn if_test() {
        let mut locals = VarMap::default();
        let mut ctx = VarTable::new(get_runtime().await, Fallback::new(None, None), &mut locals);
        assert_eq!(
            ProgramParser::new()
                .parse(
                    r##"
                            if(1 + 1 + 4 + 5 + 1 + 4 == 16, "sodayo", ~)
                        "##
                )
                .ok()
                .call(&mut ctx)
                .await
                .get_num(),
            6
        );
        assert_eq!(
            ProgramParser::new()
                .parse(
                    r##"
                            if(true, "sodayo")
                        "##
                )
                .ok()
                .call(&mut ctx)
                .await
                .get_str(),
            "sodayo"
        );
    }

    #[tokio::test]
    async fn format() {
        let mut locals = VarMap::default();
        let mut ctx = VarTable::new(get_runtime().await, Fallback::new(None, None), &mut locals);
        assert_eq!(
            ProgramParser::new()
                .parse(
                    r##"
                            format.fmt("Hello {}!", 114514)
                        "##
                )
                .ok()
                .call(&mut ctx)
                .await
                .get_str(),
            "Hello 114514!"
        )
    }
}
