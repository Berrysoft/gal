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
pub struct NativeContext {
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
type SafeHandle<'a> = &'a RwLock<NativeContext>;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct CallbackData(*mut c_void);

unsafe impl Send for CallbackData {}
unsafe impl Sync for CallbackData {}

pub type MainCallback = Option<extern "C" fn(Handle, CallbackData) -> i32>;

fn gal_main_impl(ident: *const u16, main: MainCallback, data: CallbackData) -> Result<i32> {
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
pub extern "C" fn gal_main(ident: *const u16, main: MainCallback, data: CallbackData) -> i32 {
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

pub type OpenGameCallback = Option<extern "C" fn(OpenGameStatus, *const c_void, CallbackData)>;

async fn gal_open_game_impl(
    h: SafeHandle<'_>,
    config: String,
    callback: impl Fn(OpenGameStatus, *const c_void) + Send,
) -> Result<()> {
    let mut h = h.write().await;
    {
        callback(OpenGameStatus::LoadSettings, null_mut());
        h.settings = Some(load_settings(&h.ident).await.unwrap_or_else(|e| {
            warn!("Load settings failed: {}", e);
            Default::default()
        }));
    }
    {
        let context = Context::open(&config, FrontendType::Html);
        tokio::pin!(context);
        while let Some(progress) = context.next().await {
            match progress {
                OpenStatus::LoadProfile => callback(OpenGameStatus::LoadProfile, null_mut()),
                OpenStatus::CreateRuntime => callback(OpenGameStatus::CreateRuntime, null_mut()),
                OpenStatus::LoadPlugin(name, index, len) => {
                    let name = unsafe { U16CString::from_str_unchecked(&name) };
                    let status_data = OpenGameLoadPlugin {
                        name: name.as_ptr(),
                        index,
                        len,
                    };
                    callback(OpenGameStatus::LoadPlugin, &status_data as *const _ as _)
                }
            }
        }
        let context = context.await??;
        callback(OpenGameStatus::LoadRecords, null_mut());
        h.records = load_records(&h.ident, &context.game.title)
            .await
            .unwrap_or_else(|e| {
                warn!("Load records failed: {}", e);
                Default::default()
            });
        h.context = Some(context);
        callback(OpenGameStatus::Loaded, null_mut());
    }
    Ok(())
}

#[no_mangle]
pub unsafe extern "C" fn gal_open_game(
    h: Handle,
    config: *const u16,
    callback: OpenGameCallback,
    data: CallbackData,
) {
    tokio::spawn(gal_open_game_impl(
        h.as_ref().unwrap(),
        string_from_ptr(config),
        move |status, status_data| {
            if let Some(callback) = callback {
                callback(status, status_data, data)
            }
        },
    ));
}
