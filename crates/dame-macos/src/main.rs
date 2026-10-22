//! Dame for macOS: a native `AppKit` front end for Hub draughts engines.
//!
//! The rules come from the `dame` crate; the engine runs as a separate
//! process that speaks the Hub protocol (`dame hub` by default).

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod engine;
mod input;
mod line;
#[cfg(target_os = "macos")]
// Subclassing AppKit classes and calling Objective-C needs `unsafe`.
#[allow(unsafe_code)]
mod macos;

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
#[allow(clippy::print_stderr)]
fn main() {
    eprintln!("dame-macos runs on macOS only; use `dame play` in a terminal.");
    std::process::exit(1);
}
