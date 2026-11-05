use std::process::ExitCode;

use clap::Parser;
use phone_use::commands::{Cli, run};

fn main() -> ExitCode {
    run(Cli::parse())
}
