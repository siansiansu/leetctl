#![doc = include_str!("../README.md")]
#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;

// show docs
pub mod cache;
pub mod cli;
pub mod cmd;
pub mod config;
pub mod err;
pub mod filters;
pub mod helper;
pub mod plugins;
#[cfg(feature = "pym")]
pub mod pym;
pub mod scaffold;
pub mod sets;
pub mod srs;
pub mod tui;

// re-exports
pub use cache::Cache;
pub use config::Config;
pub use err::{Error, Result};
