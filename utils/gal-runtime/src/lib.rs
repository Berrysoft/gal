//! The runtime of `gal` project.
//!
//! This runtime provides the game config, running context,
//! plugin system and settings system.
//! It can be treated as the "backend" of the game engine.

#![warn(missing_docs)]
#![deny(unsafe_code)]
#![feature(absolute_path)]
#![feature(generators)]
#![feature(round_char_boundary)]

mod config;
mod context;
pub mod plugin;
pub mod script;
mod settings;

pub use anyhow;
pub use config::*;
pub use context::*;
pub use futures_util::{pin_mut, StreamExt, TryStreamExt};
pub use gal_script::{log, RawValue};
pub use locale::*;
pub use settings::*;
pub use stream_future::*;
