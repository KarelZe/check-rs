//! The `dame` binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(dame_cli::main(std::env::args_os()))
}
