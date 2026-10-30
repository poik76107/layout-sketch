#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(layout_sketch::cli::run_cli(std::env::args_os().skip(1)))
}
