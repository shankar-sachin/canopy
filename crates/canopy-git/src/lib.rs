//! Git backend for Canopy.
//!
//! Every operation shells out to the real `git` binary and parses its
//! machine-readable output. Each operation also carries the equivalent shell
//! command so the UI can show it in teach mode.

pub mod cli;
pub mod model;
pub mod ops;
pub mod parse;

pub use cli::{Git, GitError, Output};
pub use model::*;
