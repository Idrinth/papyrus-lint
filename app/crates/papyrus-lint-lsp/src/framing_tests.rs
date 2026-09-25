use std::io::Cursor;

use super::{read_message, write_message};

#[test]
fn round_trips_a_body() {
    let mut encoded = Vec::new();
    write_message(&mut encoded, br#"{"jsonrpc":"2.0"}"#).unwrap();
    let mut reader = Cursor::new(encoded);
    let body = read_message(&mut reader).unwrap().unwrap();
    assert_eq!(body, br#"{"jsonrpc":"2.0"}"#);
    assert!(read_message(&mut reader).unwrap().is_none());
}

#[test]
fn accepts_lowercase_content_length_and_ignores_content_type() {
    let raw =
        b"content-type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: 2\r\n\r\n{}";
    let mut reader = Cursor::new(&raw[..]);
    assert_eq!(read_message(&mut reader).unwrap().unwrap(), b"{}");
}

#[test]
fn eof_before_any_header_is_a_clean_end() {
    let mut reader = Cursor::new(&b""[..]);
    assert!(read_message(&mut reader).unwrap().is_none());
}

#[test]
fn missing_content_length_is_an_error() {
    let mut reader = Cursor::new(&b"Content-Type: application/vscode-jsonrpc\r\n\r\n"[..]);
    let error = read_message(&mut reader).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn rejects_a_non_numeric_content_length() {
    let mut reader = Cursor::new(&b"Content-Length: no\r\n\r\n"[..]);
    assert!(read_message(&mut reader).is_err());
}

#[test]
fn rejects_malformed_and_truncated_messages() {
    let mut malformed = Cursor::new(&b"Content-Length 2\r\n\r\n{}"[..]);
    assert_eq!(
        read_message(&mut malformed).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );

    let mut headers = Cursor::new(&b"Content-Length: 2\r\n"[..]);
    assert_eq!(
        read_message(&mut headers).unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );

    let mut body = Cursor::new(&b"Content-Length: 3\r\n\r\n{}"[..]);
    assert_eq!(
        read_message(&mut body).unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}
