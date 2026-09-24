use std::io::{self, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    match papyrus_lint_lsp::serve(stdin, stdout) {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("papyrus-lint-lsp: {error}");
            ExitCode::from(1)
        }
    }
}
