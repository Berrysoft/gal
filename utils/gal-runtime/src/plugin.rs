//! The plugin utilities.

#![allow(unsafe_code)]
#![allow(clippy::mut_from_ref)]

use crate::*;
use anyhow::{bail, Result};
use gal_bindings_types::*;
use log::warn;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use wasmer::*;
use wasmer_wasi::*;

/// An instance of a WASM plugin module.
pub struct Host {
    abi_free: NativeFunc<(i32, i32, i32), ()>,
    abi_alloc: NativeFunc<(i32, i32), i32>,
    export_free: NativeFunc<(i32, i32), ()>,
    export_async_poll: NativeFunc<(u64, i32), u64>,
    export_async_free: NativeFunc<u64, ()>,
    instance: Instance,
}

unsafe fn mem_slice(memory: &Memory, start: i32, len: i32) -> &[u8] {
    memory
        .data_unchecked()
        .get_unchecked(start as usize..)
        .get_unchecked(..len as usize)
}

unsafe fn mem_slice_mut(memory: &Memory, start: i32, len: i32) -> &mut [u8] {
    memory
        .data_unchecked_mut()
        .get_unchecked_mut(start as usize..)
        .get_unchecked_mut(..len as usize)
}

impl Host {
    /// Loads the WASM [`Module`], with some imports.
    pub fn new(module: &Module, resolver: &(dyn Resolver + Send + Sync)) -> Result<Self> {
        let instance = Instance::new(module, resolver)?;
        let abi_free = instance.exports.get_native_function("__abi_free")?;
        let abi_alloc = instance.exports.get_native_function("__abi_alloc")?;
        let export_free = instance.exports.get_native_function("__export_free")?;
        let export_async_poll = instance
            .exports
            .get_native_function("__export_async_poll")?;
        let export_async_free = instance
            .exports
            .get_native_function("__export_async_free")?;
        Ok(Self {
            abi_free,
            abi_alloc,
            export_free,
            export_async_poll,
            export_async_free,
            instance,
        })
    }

    /// Calls a method by name.
    ///
    /// The args and returns are passed by MessagePack with [`rmp_serde`].
    pub async fn call<Params: Serialize, Res: DeserializeOwned>(
        &self,
        name: &str,
        args: Params,
    ) -> Result<Res> {
        let memory = self.instance.exports.get_memory("memory")?;
        let data = rmp_serde::to_vec(&args)?;
        let ptr = self.abi_alloc.call(8, data.len() as i32)?;
        unsafe { mem_slice_mut(memory, ptr, data.len() as i32) }.copy_from_slice(&data);
        let res = if let Ok(func) = self.instance.exports.get_native_function(name) {
            self.call_impl(memory, func, ptr, data.len() as i32)?
        } else if let Ok(func) = self
            .instance
            .exports
            .get_native_function(&format!("{}_async", name))
        {
            self.call_async_impl(memory, func, ptr, data.len() as i32)
                .await?
        } else {
            bail!("Cannot find function {}", name);
        };
        self.abi_free.call(ptr, data.len() as i32, 8)?;
        Ok(res)
    }

    fn call_impl<Res: DeserializeOwned>(
        &self,
        memory: &Memory,
        func: NativeFunc<(i32, i32), u64>,
        ptr: i32,
        len: i32,
    ) -> Result<Res> {
        let res = func.call(len, ptr)?;
        let (len, res) = ((res >> 32) as i32, (res & 0xFFFFFFFF) as i32);
        let res_data = unsafe { mem_slice(memory, res, len) };
        let res_data = rmp_serde::from_slice(res_data)?;
        self.export_free.call(len, res)?;
        Ok(res_data)
    }

    async fn call_async_impl<Res: DeserializeOwned>(
        &self,
        memory: &Memory,
        func: NativeFunc<(i32, i32), u64>,
        ptr: i32,
        len: i32,
    ) -> Result<Res> {
        let fut = func.call(len, ptr)?;
        let wasm_addr = self.abi_alloc.call(8, 8)?;
        let wasm_ptr = unsafe { memory.data_ptr().add(wasm_addr as usize) } as usize;
        let wasm_fut = WasmFuture {
            fut,
            wasm_addr,
            wasm_ptr,
            export_async_poll: self.export_async_poll.clone(),
        };
        let res = wasm_fut.await;
        let (len, res) = ((res >> 32) as i32, (res & 0xFFFFFFFF) as i32);
        let res_data = unsafe { mem_slice(memory, res, len) };
        let res_data = rmp_serde::from_slice(res_data)?;
        self.export_free.call(len, res)?;
        self.export_async_free.call(fut)?;
        self.abi_free.call(wasm_addr, 8, 8)?;
        Ok(res_data)
    }

    /// Calls a script plugin method by name.
    pub async fn dispatch_method(&self, name: &str, args: &[RawValue]) -> Result<RawValue> {
        self.call(name, (args,)).await
    }

    /// Gets the [`PluginType`].
    pub async fn plugin_type(&self) -> Result<PluginType> {
        self.call("plugin_type", ()).await
    }

    /// Processes [`Action`] in action plugin.
    pub async fn process_action(&self, ctx: ActionProcessContextRef<'_>) -> Result<Action> {
        self.call("process_action", (ctx,)).await
    }

    /// Gets registered TeX commands of a text plugin.
    pub async fn text_commands(&self) -> Result<Vec<String>> {
        self.call("text_commands", ()).await
    }

    /// Calls a custom command in the text plugin.
    pub async fn dispatch_command(
        &self,
        name: &str,
        args: &[String],
        ctx: TextProcessContextRef<'_>,
    ) -> Result<TextProcessResult> {
        self.call(name, (args, ctx)).await
    }

    /// Processes [`Game`] when opening the config file.
    pub async fn process_game(&self, ctx: GameProcessContextRef<'_>) -> Result<GameProcessResult> {
        self.call("process_game", (ctx,)).await
    }
}

/// The plugin runtime.
pub struct Runtime {
    /// The plugins map by name.
    pub modules: HashMap<String, Host>,
    /// The action plugins.
    pub action_modules: Vec<String>,
    /// The text plugins by command name.
    pub text_modules: HashMap<String, String>,
    /// The game plugins.
    pub game_modules: Vec<String>,
}

/// The load status of [`Runtime`].
#[derive(Debug, Clone)]
pub enum LoadStatus {
    /// Start creating the engine.
    CreateEngine,
    /// Loading the plugin.
    LoadPlugin(
        /// Plugin name.
        String,
        /// Plugin index.
        usize,
        /// Plugin total count.
        usize,
    ),
}

#[derive(Default, Clone, WasmerEnv)]
struct RuntimeInstanceData {
    #[wasmer(export)]
    memory: LazyInit<Memory>,
}

impl Runtime {
    fn imports(store: &Store) -> Result<Box<dyn NamedResolver + Send + Sync>> {
        let log_func = Function::new_native_with_env(
            store,
            RuntimeInstanceData::default(),
            |env_data: &RuntimeInstanceData, len: i32, data: i32| {
                let memory = unsafe { env_data.memory.get_unchecked() };
                let data = unsafe { mem_slice(memory, data, len) };
                let data: Record = rmp_serde::from_slice(data).unwrap();
                log::logger().log(
                    &log::Record::builder()
                        .level(data.level)
                        .target(&data.target)
                        .args(format_args!("{}", data.msg))
                        .module_path(data.module_path.as_deref())
                        .file(data.file.as_deref())
                        .line(data.line)
                        .build(),
                );
            },
        );
        let log_flush_func = Function::new_native(store, || log::logger().flush());

        let async_wake_func = Function::new_native_with_env(
            store,
            RuntimeInstanceData::default(),
            |env_data: &RuntimeInstanceData, waker: i32| {
                let memory = unsafe { env_data.memory.get_unchecked() };
                let ptr = unsafe { memory.data_ptr().add(waker as usize) } as *mut usize;
                let waker = unsafe { *ptr } as *mut Waker;
                unsafe { waker.as_ref() }.expect("__wake").wake_by_ref();
            },
        );

        let import_object = imports! {
            "log" => {
                "__log" => log_func,
                "__log_flush" => log_flush_func,
            },
            "async" => {
                "__wake" => async_wake_func,
            }
        };
        let wasi_env = WasiState::new("gal-runtime").preopen_dir("/")?.finalize()?;
        let wasi_import = generate_import_object_from_env(store, wasi_env, WasiVersion::Latest);
        Ok(Box::new(import_object.chain_front(wasi_import)))
    }

    /// Load plugins from specific directory and plugin names.
    ///
    /// The actual load folder will be `rel_to.join(dir)`.
    ///
    /// If `names` is empty, all WASM files will be loaded.
    #[stream(LoadStatus, lifetime = "'a")]
    pub async fn load<'a>(
        dir: impl AsRef<Path> + 'a,
        rel_to: impl AsRef<Path> + 'a,
        names: &'a [impl AsRef<str>],
    ) -> Result<Self> {
        let path = rel_to.as_ref().join(dir);
        yield LoadStatus::CreateEngine;
        let store = Store::default();
        let import_object = Self::imports(&store)?;
        let mut modules = HashMap::new();
        let mut action_modules = vec![];
        let mut text_modules = HashMap::new();
        let mut game_modules = vec![];
        let mut paths = vec![];
        if names.is_empty() {
            let mut dirs = ReadDirStream::new(tokio::fs::read_dir(path).await?);
            while let Some(f) = dirs.try_next().await? {
                let p = f.path();
                if p.extension()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_default()
                    == "wasm"
                {
                    let name = p
                        .file_stem()
                        .map(|s| s.to_string_lossy())
                        .unwrap_or_default()
                        .into_owned();
                    paths.push((name, p));
                }
            }
        } else {
            for name in names {
                let p = path.join(name.as_ref()).with_extension("wasm");
                if p.exists() {
                    paths.push((name.as_ref().to_string(), p));
                }
            }
        }
        let total_len = paths.len();
        for (i, (name, p)) in paths.into_iter().enumerate() {
            yield LoadStatus::LoadPlugin(name.clone(), i, total_len);
            let buf = tokio::fs::read(&p).await?;
            let module = Module::from_binary(&store, &buf)?;
            let runtime = Host::new(&module, &import_object)?;
            let plugin_type = runtime.plugin_type().await?;
            if plugin_type.contains(PluginType::ACTION) {
                action_modules.push(name.clone());
            }
            if plugin_type.contains(PluginType::TEXT) {
                let cmds = runtime.text_commands().await?;
                for cmd in cmds.into_iter() {
                    let res = text_modules.insert(cmd.clone(), name.clone());
                    if let Some(old_module) = res {
                        warn!(
                            "Command `{}` is overrided by \"{}\" over \"{}\"",
                            cmd, name, old_module
                        );
                    }
                }
            }
            if plugin_type.contains(PluginType::GAME) {
                game_modules.push(name.clone());
            }
            modules.insert(name, runtime);
        }
        Ok(Self {
            modules,
            action_modules,
            text_modules,
            game_modules,
        })
    }
}

struct WasmFuture {
    pub fut: u64,
    pub wasm_addr: i32,
    pub wasm_ptr: usize,
    pub export_async_poll: NativeFunc<(u64, i32), u64>,
}

impl Future for WasmFuture {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = cx.waker();
        let waker_addr = std::ptr::addr_of!(*waker);
        unsafe { (self.wasm_ptr as *mut usize).write_unaligned(waker_addr as usize) };
        match self
            .export_async_poll
            .call(self.fut, self.wasm_addr)
            .expect("__poll")
        {
            std::u64::MAX => Poll::Pending,
            v => Poll::Ready(v),
        }
    }
}
