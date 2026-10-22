//! Command-line front end for the dame draughts engine.

use clap::Parser;

/// A fast international draughts engine.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
