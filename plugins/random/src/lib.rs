use gal_bindings::*;
use log::error;
use num_bigint::RandBigInt;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Mutex;

#[export]
fn plugin_type() -> PluginType {
    PluginType::Script
}

lazy_static::lazy_static! {
    static ref RNG: Mutex<StdRng> = Mutex::new(StdRng::from_entropy());
}

#[export]
fn rnd(args: Vec<RawValue>) -> RawValue {
    if let Ok(mut rng) = RNG.lock() {
        let res = match args.len() {
            0 => rng.gen::<i16>().into(),
            1 => rng.gen_bigint_range(&0.into(), &args[0].get_num()),
            _ => rng.gen_bigint_range(&args[0].get_num(), &args[1].get_num()),
        };
        RawValue::Num(res)
    } else {
        error!("Cannot get random engine.");
        RawValue::Unit
    }
}
