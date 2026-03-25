//! posting-import library
//!
//! This library provides utilities for importing API collections from
//! Postman, Insomnia, and Bruno to the Posting TUI format.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;
pub mod core;
pub mod io;
pub mod plugins;

pub use cli::{run, Cli};
pub use core::models::{Collection, Request, SourceType};
pub use io::{PostingWriter, WriterConfig};
pub use plugins::{default_registry, ImporterPlugin, PluginInfo, PluginRegistry};
