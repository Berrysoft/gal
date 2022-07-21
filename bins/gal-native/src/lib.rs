use anyhow::Result;
use gal_runtime::*;
use log::warn;
use std::{ffi::c_void, ptr::null_mut};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use widestring::{U16CStr, U16CString};

unsafe fn string_from_ptr(p: *const u16) -> String {
    if !p.is_null() {
        U16CStr::from_ptr_str(p).to_string_lossy()
    } else {
        String::new()
    }
}

#[derive(Default)]
struct NativeContext {
    ident: String,
    settings: Option<Settings>,
    context: Option<Context>,
    records: Vec<RawContext>,
}

impl NativeContext {
    pub fn new(ident: impl Into<String>) -> Self {
        Self {
            ident: ident.into(),
            ..Default::default()
        }
    }
}

type Handle = *const RwLock<NativeContext>;

pub type MainCallback = Option<extern "C" fn(Handle, *mut c_void) -> i32>;

fn gal_main_impl(ident: *const u16, main: MainCallback, data: *mut c_void) -> Result<i32> {
    let ident = unsafe { string_from_ptr(ident) };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        if let Some(main) = main {
            let native_context = RwLock::new(NativeContext::new(ident));
            let res = main(&native_context, data);
            anyhow::Ok(res)
        } else {
            anyhow::Ok(0)
        }
    })
}

#[no_mangle]
pub extern "C" fn gal_main(ident: *const u16, main: MainCallback, data: *mut c_void) -> i32 {
    gal_main_impl(ident, main, data).unwrap_or(1)
}

#[repr(C)]
pub enum OpenGameStatus {
    LoadSettings,
    LoadProfile,
    CreateRuntime,
    LoadPlugin,
    LoadRecords,
    Loaded,
}

#[repr(C)]
pub struct OpenGameLoadPlugin {
    pub name: *const u16,
    pub index: usize,
    pub len: usize,
}

pub type OpenGameCallback = Option<extern "C" fn(OpenGameStatus, *mut c_void, *mut c_void)>;

fn emit_open_game_callback<T>(
    status: OpenGameStatus,
    mut status_data: Option<T>,
    callback: OpenGameCallback,
    data: *mut c_void,
) {
    if let Some(callback) = callback {
        callback(
            status,
            if let Some(status_data) = status_data.as_mut() {
                status_data as *mut T as _
            } else {
                null_mut()
            },
            data,
        )
    }
}

async fn gal_open_game_impl(
    h: Handle,
    config: *const u16,
    callback: OpenGameCallback,
    data: *mut c_void,
) -> Result<()> {
    let mut h = unsafe { h.write().await };
    {
        emit_open_game_callback(OpenGameStatus::LoadSettings, None, callback, data);
        h.settings = Some(load_settings(&h.ident).await.unwrap_or_else(|e| {
            warn!("Load settings failed: {}", e);
            Default::default()
        }));
    }
    {
        let config = unsafe { string_from_ptr(config) };
        let context = Context::open(&config, FrontendType::Html);
        tokio::pin!(context);
        while let Some(progress) = context.next().await {
            match progress {
                OpenStatus::LoadProfile => {
                    emit_open_game_callback(OpenGameStatus::LoadProfile, None, callback, data)
                }
                OpenStatus::CreateRuntime => {
                    emit_open_game_callback(OpenGameStatus::CreateRuntime, None, callback, data)
                }
                OpenStatus::LoadPlugin(name, index, len) => {
                    let name = unsafe { U16CString::from_str_unchecked(&name) };
                    let status_data = OpenGameLoadPlugin {
                        name: name.as_ptr(),
                        index,
                        len,
                    };
                    emit_open_game_callback(
                        OpenGameStatus::LoadPlugin,
                        Some(status_data),
                        callback,
                        data,
                    )
                }
            }
        }
        let context = context.await??;
        emit_open_game_callback(OpenGameStatus::LoadRecords, None, callback, data);
        h.records = load_records(&h.ident, &context.game.title)
            .await
            .unwrap_or_else(|e| {
                warn!("Load records failed: {}", e);
                Default::default()
            });
        h.context = Some(context);
        emit_open_game_callback(OpenGameStatus::Loaded, None, callback, data);
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn gal_open_game(
    h: Handle,
    config: *const u16,
    callback: OpenGameCallback,
    data: *mut c_void,
) {
    tokio::spawn(gal_open_game_impl(h, config, callback, data))
}
