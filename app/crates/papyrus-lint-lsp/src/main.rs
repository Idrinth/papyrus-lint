use std::io::{self, BufRead, BufReader, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    run(stdin, stdout)
}

fn run(input: impl BufRead, output: impl Write) -> ExitCode {
    match papyrus_lint_lsp::serve(input, output) {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("papyrus-lint-lsp: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::process::ExitCode;

    use super::run;

    #[test]
    fn clean_eof_exits_successfully() {
        assert_eq!(run(Cursor::new(Vec::new()), Vec::new()), ExitCode::SUCCESS);
    }

    #[test]
    fn exit_notification_before_shutdown_returns_failure() {
        let message = br#"{"jsonrpc":"2.0","method":"exit"}"#;
        let input = framed(message);

        assert_eq!(run(Cursor::new(input), Vec::new()), ExitCode::FAILURE);
    }

    #[test]
    fn input_error_returns_failure() {
        let input = Cursor::new(b"Content-Length: invalid\r\n\r\n".to_vec());

        assert_eq!(run(input, Vec::new()), ExitCode::FAILURE);
    }

    fn framed(message: &[u8]) -> Vec<u8> {
        let mut framed = format!("Content-Length: {}\r\n\r\n", message.len()).into_bytes();
        framed.extend_from_slice(message);
        framed
    }
}
