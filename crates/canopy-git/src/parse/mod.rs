//! Parsers for git's machine-readable output formats.

pub mod conflict;
pub mod diff;
pub mod log;
pub mod refs;
pub mod status;

/// Field separator used in our custom `--format` strings.
pub const FS: char = '\x1f';
/// Record separator used in our custom `--format` strings.
pub const RS: char = '\x1e';
