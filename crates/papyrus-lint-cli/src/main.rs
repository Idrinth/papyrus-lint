use std::env;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = papyrus_lint_cli::run(&args, &mut io::stdout(), &mut io::stderr());
    ExitCode::from(code)
}
