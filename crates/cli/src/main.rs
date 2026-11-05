use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("version") => {
            println!("phone-use {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: phone-use <command>\n\ncommands:\n  version");
            ExitCode::from(2)
        }
    }
}
