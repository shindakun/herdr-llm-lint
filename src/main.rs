use std::process::ExitCode;

use herdr_llm_lint::cli::{self, USAGE};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("lint") => cli::lint(&args[1..]),
        Some("herdr-action") => cli::herdr_action(),
        Some("herdr-send") => cli::herdr_send(&args[1..]),
        Some("herdr-event") => cli::herdr_event(),
        Some("herdr-pane") => cli::herdr_pane(&args[1..]),
        None | Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown subcommand: {other}\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("herdr-llm-lint: {err}");
            ExitCode::FAILURE
        }
    }
}
