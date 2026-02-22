pub mod config;
pub mod api;
pub mod search;
pub mod downloader;
pub mod export;
pub mod cli;
pub mod batch;
pub mod error;
pub mod utils;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
