use std::process::ExitCode;

use clap::FromArgMatches;
use phone_use::commands::{Cli, command, run};

fn main() -> ExitCode {
    let matches = command().get_matches();
    run(Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit()))
}
