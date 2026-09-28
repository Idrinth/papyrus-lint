use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_server(input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_PapyrusLinterLsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn language server");
    child
        .stdin
        .take()
        .expect("capture language server stdin")
        .write_all(input)
        .expect("write language server input");
    child.wait_with_output().expect("wait for language server")
}

fn framed(message: &[u8]) -> Vec<u8> {
    let mut framed = format!("Content-Length: {}\r\n\r\n", message.len()).into_bytes();
    framed.extend_from_slice(message);
    framed
}

#[test]
fn binary_exits_successfully_at_clean_eof() {
    let output = run_server(&[]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn binary_forwards_protocol_output_to_stdout() {
    let output = run_server(&framed(
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    ));

    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"Content-Length: "));
    assert!(output
        .stdout
        .windows(b"papyrus-lint-lsp".len())
        .any(|window| window == b"papyrus-lint-lsp"));
    assert!(output.stderr.is_empty());
}

#[test]
fn binary_reports_protocol_input_errors_to_stderr() {
    let output = run_server(b"Content-Length: invalid\r\n\r\n");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "papyrus-lint-lsp: invalid Content-Length\n"
    );
}
