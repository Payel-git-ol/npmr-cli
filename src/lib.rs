pub mod cli;
pub mod commands;
pub mod npm;
pub mod output;
pub mod store;
pub mod utils;

pub use npm::Npm;
pub use npm::NpmError;

pub const VERSION: &str = "12.2.1";
pub const NPM_PATH: &str = "npmr";
