//! A fast international draughts (10x10 checkers) engine.
//!
//! The crate is the engine core: board representation, move generation,
//! search and evaluation. It has no dependencies and no Python coupling; the
//! `dame-cli` and `dame-python` crates build on top of it.

/// The crate version, as declared in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
