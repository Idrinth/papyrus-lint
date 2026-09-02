use std::env;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();
    let code = papyrus_lint_cli::run(&args, &mut stdout, &mut io::stderr(), stdout_is_terminal);
    ExitCode::from(code)
}
