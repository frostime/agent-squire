//! Shared leaf modules used by multiple built-in commands.
//!
//! Modules here contain reusable domain or technical primitives without CLI
//! lifecycle ownership. They must not depend on `builtins`, `runtime`, `cli`,
//! or `external`.

pub mod encoding;
pub mod file_sources;
pub mod markdown;
pub mod path;
