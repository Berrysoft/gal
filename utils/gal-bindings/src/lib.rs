#![feature(fn_traits)]
#![feature(ptr_metadata)]
#![feature(unboxed_closures)]

pub use gal_bindings_types::*;
pub use gal_primitive::*;
pub use log;

mod logger;

use serde::{de::DeserializeOwned, Serialize};
use std::{
    alloc::{self, Layout},
    future::Future,
    mem::transmute,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[no_mangle]
unsafe extern "C" fn __abi_alloc(align: usize, new_len: usize) -> *mut u8 {
    if new_len == 0 {
        return align as *mut u8;
    }
    let layout = Layout::from_size_align_unchecked(new_len, align);
    let ptr = alloc::alloc(layout);
    if ptr.is_null() {
        alloc::handle_alloc_error(layout);
    }
    return ptr;
}

#[no_mangle]
unsafe extern "C" fn __abi_free(ptr: *mut u8, len: usize, align: usize) {
    if len == 0 {
        return;
    }
    let layout = Layout::from_size_align_unchecked(len, align);
    alloc::dealloc(ptr, layout);
}

unsafe fn return_slice(data: &[u8]) -> u64 {
    let len = data.len();
    let ptr = data.as_ptr();
    ((len as u64) << 32) | (ptr as u64)
}

unsafe fn return_future(data: &dyn Future<Output = u64>) -> u64 {
    let (ptr, meta) = std::ptr::addr_of!(*data).to_raw_parts();
    ((transmute::<_, usize>(meta) as u64) << 32) | (ptr as u64)
}

#[doc(hidden)]
pub unsafe fn __export<Params: DeserializeOwned, Res: Serialize>(
    len: usize,
    data: *const u8,
    f: impl FnOnce<Params, Output = Res>,
) -> u64 {
    logger::PluginLogger::init();
    let data = std::slice::from_raw_parts(data, len);
    let data = rmp_serde::from_slice(data).unwrap();
    let res = f.call_once(data);
    let data = rmp_serde::to_vec(&res).unwrap();
    let data = data.into_boxed_slice();
    let data = Box::leak(data);
    return_slice(data)
}

#[no_mangle]
unsafe extern "C" fn __export_free(len: usize, data: *mut u8) {
    let _data = Box::from_raw(std::slice::from_raw_parts_mut(data, len));
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "async")]
extern "C" {
    fn __wake(waker: *const ());
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn __wake(_waker: *const ()) {}

unsafe fn __async_waker_clone(waker: *const ()) -> RawWaker {
    RawWaker::new(waker, &WAKER_VTABLE)
}

unsafe fn __async_waker_wake(waker: *const ()) {
    __wake(waker)
}

unsafe fn __async_waker_drop(_waker: *const ()) {}

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    __async_waker_clone,
    __async_waker_wake,
    __async_waker_wake,
    __async_waker_drop,
);

#[doc(hidden)]
pub unsafe fn __export_async<
    Params: DeserializeOwned + std::fmt::Debug,
    Res: Serialize,
    F: Future<Output = Res>,
>(
    len: usize,
    data: *const u8,
    f: impl FnOnce<Params, Output = F>,
) -> u64 {
    logger::PluginLogger::init();
    let data = std::slice::from_raw_parts(data, len);
    let data = rmp_serde::from_slice(data).unwrap();
    let fut = async {
        let res = f.call_once(data).await;
        let data = rmp_serde::to_vec(&res).unwrap();
        let data = data.into_boxed_slice();
        let data = Box::leak(data);
        return_slice(data)
    };
    let fut: Box<dyn Future<Output = u64>> = Box::new(fut);
    let fut = Box::leak(fut);
    return_future(fut)
}

#[no_mangle]
unsafe extern "C" fn __export_async_poll(data: u64, waker: *const ()) -> u64 {
    let (meta, data) = ((data >> 32) as i32, (data & 0xFFFFFFFF) as *mut ());
    let fut: *mut (dyn Future<Output = u64>) = std::ptr::from_raw_parts_mut(data, transmute(meta));
    let fut = Pin::new_unchecked(fut.as_mut().unwrap());
    let waker = Waker::from_raw(RawWaker::new(waker, &WAKER_VTABLE));
    let mut cx = Context::from_waker(&waker);
    match fut.poll(&mut cx) {
        Poll::Pending => std::u64::MAX,
        Poll::Ready(v) => v,
    }
}

#[no_mangle]
unsafe extern "C" fn __export_async_free(data: u64) {
    let (meta, data) = ((data >> 32) as i32, (data & 0xFFFFFFFF) as *mut ());
    let fut: *mut (dyn Future<Output = u64>) = std::ptr::from_raw_parts_mut(data, transmute(meta));
    let _fut = Box::from_raw(fut);
}

pub use gal_bindings_impl::export;
