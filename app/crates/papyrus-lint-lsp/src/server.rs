use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::code_actions;
use crate::documents::Documents;
use crate::framing::{read_message, write_message};
use crate::SERVER_NAME;

/// Command id reserved for a later whole-file fix. `workspace/executeCommand`
/// accepts it and returns null until the fix pipeline is wired in.
pub const FIX_FILE_COMMAND: &str = "papyrusLint.fixFile";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const SERVER_NOT_INITIALIZED: i64 = -32002;

/// Reads LSP messages from `input` and writes responses to `output`.
///
/// Returns the process exit code: `1` when the client sends `exit` before
/// `shutdown`, otherwise `0`.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<i32> {
    let mut shutdown = false;
    let mut initialized = false;
    let mut documents = Documents::default();
    loop {
        let Some(bytes) = read_message(&mut input)? else {
            return Ok(0);
        };
        let message: Value = match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) if value.is_object() => value,
            Ok(_) => {
                write_error(&mut output, None, INVALID_REQUEST, "Invalid Request")?;
                continue;
            }
            Err(_) => {
                write_error(&mut output, None, PARSE_ERROR, "Parse error")?;
                continue;
            }
        };
        let id = message.get("id").filter(|id| !id.is_null()).cloned();
        let method = message.get("method").and_then(Value::as_str);
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        if id.is_none() {
            if method == Some("exit") {
                return Ok(if shutdown { 0 } else { 1 });
            }
            match method {
                Some("textDocument/didOpen") => documents.did_open(&params, &mut output)?,
                Some("textDocument/didChange") => documents.did_change(&params, &mut output)?,
                Some("textDocument/didSave") => documents.did_save(&params, &mut output)?,
                Some("textDocument/didClose") => documents.did_close(&params, &mut output)?,
                _ => {}
            }
            continue;
        }
        let Some(method) = method else {
            write_error(&mut output, id.as_ref(), INVALID_REQUEST, "Invalid Request")?;
            continue;
        };
        if shutdown {
            write_error(
                &mut output,
                id.as_ref(),
                INVALID_REQUEST,
                "Server is shut down",
            )?;
            continue;
        }
        if !initialized && method != "initialize" {
            write_error(
                &mut output,
                id.as_ref(),
                SERVER_NOT_INITIALIZED,
                "Server not initialized",
            )?;
            continue;
        }
        if initialized && method == "initialize" {
            write_error(&mut output, id.as_ref(), INVALID_REQUEST, "Invalid Request")?;
            continue;
        }
        match method {
            "initialize" => {
                initialized = true;
                write_result(&mut output, id.as_ref(), initialize_result())?;
            }
            "shutdown" => {
                shutdown = true;
                write_result(&mut output, id.as_ref(), Value::Null)?;
            }
            "textDocument/codeAction" => write_result(
                &mut output,
                id.as_ref(),
                code_actions::provide(&documents, &params),
            )?,
            "workspace/executeCommand" => {
                write_result(&mut output, id.as_ref(), Value::Null)?;
            }
            _ => write_error(
                &mut output,
                id.as_ref(),
                METHOD_NOT_FOUND,
                "Method not found",
            )?,
        }
    }
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": { "includeText": true }
            },
            "codeActionProvider": true,
            "executeCommandProvider": {
                "commands": [FIX_FILE_COMMAND]
            }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn write_result(output: &mut impl Write, id: Option<&Value>, result: Value) -> io::Result<()> {
    let body = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id.cloned().unwrap_or(Value::Null),
        "result": result,
    }))
    .expect("response json");
    write_message(output, &body)
}

fn write_error(
    output: &mut impl Write,
    id: Option<&Value>,
    code: i64,
    message: &str,
) -> io::Result<()> {
    let body = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id.cloned().unwrap_or(Value::Null),
        "error": { "code": code, "message": message }
    }))
    .expect("error json");
    write_message(output, &body)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
