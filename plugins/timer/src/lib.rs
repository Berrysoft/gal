#![feature(once_cell)]

use gal_bindings::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    thread::sleep as thread_sleep,
    time::Duration,
};

#[export]
fn plugin_type() -> PluginType {
    PluginType::SCRIPT
}

#[export]
async fn sleep(args: Vec<RawValue>) -> RawValue {
    if let Some(arg) = args.get(0) {
        let period = Duration::from_millis(arg.get_num() as u64);
        Sleep::new(period).await;
    }
    RawValue::Unit
}

struct Sleep {
    period: Duration,
}

impl Sleep {
    pub fn new(period: Duration) -> Self {
        Self { period }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        thread_sleep(self.period);
        cx.waker().wake_by_ref();
        Poll::Ready(())
    }
}
