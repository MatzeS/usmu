use std::process::ExitCode;

use clap::Parser;
use usmu::record_iv_curve::CommandlineArguments;
fn main() -> ExitCode {
    let result = CommandlineArguments::parse().run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
