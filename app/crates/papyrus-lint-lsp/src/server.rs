use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::code_actions;
use crate::documents::Documents;
use crate::framing::{read_message, write_message};
use crate::SERVER_NAME;

/// Whole-file automatic fix. The server applies it with `workspace/applyEdit`.
pub const FIX_FILE_COMMAND: &str = "papyrusLint.fixFile";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const SERVER_NOT_INITIALIZED: i64 = -32002;
const REQUEST_FAILED: i64 = -32803;

/// Reads LSP messages from `input` and writes responses to `output`.
///
/// Returns the process exit code: `1` when the client sends `exit` before
/// `shutdown`, otherwise `0`.
pub fn serve(input: impl BufRead, output: impl Write) -> io::Result<i32> {
    Server {
        input,
        output,
        shutdown: false,
        initialized: false,
        documents: Documents::default(),
        next_request: 0,
    }
    .run()
}

struct Server<R, W> {
    input: R,
    output: W,
    shutdown: bool,
    initialized: bool,
    documents: Documents,
    next_request: u64,
}

impl<R: BufRead, W: Write> Server<R, W> {
    fn run(&mut self) -> io::Result<i32> {
        loop {
            let Some(message) = self.read_message()? else {
                return Ok(0);
            };
            if let Some(code) = self.dispatch(message)? {
                return Ok(code);
            }
        }
    }

    fn read_message(&mut self) -> io::Result<Option<Value>> {
        let Some(bytes) = read_message(&mut self.input)? else {
            return Ok(None);
        };
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) if value.is_object() => Ok(Some(value)),
            Ok(_) => {
                write_error(&mut self.output, None, INVALID_REQUEST, "Invalid Request")?;
                Ok(Some(Value::Null))
            }
            Err(_) => {
                write_error(&mut self.output, None, PARSE_ERROR, "Parse error")?;
                Ok(Some(Value::Null))
            }
        }
    }

    fn dispatch(&mut self, message: Value) -> io::Result<Option<i32>> {
        if message.is_null() {
            return Ok(None);
        }
        let id = message.get("id").filter(|id| !id.is_null()).cloned();
        let method = message.get("method").and_then(Value::as_str);
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        if id.is_none() {
            if method == Some("exit") {
                return Ok(Some(if self.shutdown { 0 } else { 1 }));
            }
            match method {
                Some("textDocument/didOpen") => {
                    self.documents.did_open(&params, &mut self.output)?
                }
                Some("textDocument/didChange") => {
                    self.documents.did_change(&params, &mut self.output)?;
                }
                Some("textDocument/didSave") => {
                    self.documents.did_save(&params, &mut self.output)?
                }
                Some("textDocument/didClose") => {
                    self.documents.did_close(&params, &mut self.output)?;
                }
                _ => {}
            }
            return Ok(None);
        }
        let Some(method) = method else {
            write_error(
                &mut self.output,
                id.as_ref(),
                INVALID_REQUEST,
                "Invalid Request",
            )?;
            return Ok(None);
        };
        if self.shutdown {
            write_error(
                &mut self.output,
                id.as_ref(),
                INVALID_REQUEST,
                "Server is shut down",
            )?;
            return Ok(None);
        }
        if !self.initialized && method != "initialize" {
            write_error(
                &mut self.output,
                id.as_ref(),
                SERVER_NOT_INITIALIZED,
                "Server not initialized",
            )?;
            return Ok(None);
        }
        if self.initialized && method == "initialize" {
            write_error(
                &mut self.output,
                id.as_ref(),
                INVALID_REQUEST,
                "Invalid Request",
            )?;
            return Ok(None);
        }
        match method {
            "initialize" => {
                self.initialized = true;
                write_result(&mut self.output, id.as_ref(), initialize_result())?;
            }
            "shutdown" => {
                self.shutdown = true;
                write_result(&mut self.output, id.as_ref(), Value::Null)?;
            }
            "textDocument/codeAction" => write_result(
                &mut self.output,
                id.as_ref(),
                code_actions::provide(&self.documents, &params),
            )?,
            "workspace/executeCommand" => self.execute_command(id.as_ref(), &params)?,
            _ => write_error(
                &mut self.output,
                id.as_ref(),
                METHOD_NOT_FOUND,
                "Method not found",
            )?,
        }
        Ok(None)
    }

    fn execute_command(&mut self, id: Option<&Value>, params: &Value) -> io::Result<()> {
        let command = params["command"].as_str().unwrap_or("");
        if command != FIX_FILE_COMMAND {
            return write_error(&mut self.output, id, METHOD_NOT_FOUND, "Method not found");
        }
        let Some(uri) = crate::commands::command_uri(params) else {
            return write_result(&mut self.output, id, Value::Null);
        };
        let Some(document) = self.documents.get(&uri) else {
            return write_error(
                &mut self.output,
                id,
                INVALID_REQUEST,
                "document is not open",
            );
        };
        let Some(repaired) = crate::commands::repaired_text(&document.text, &uri) else {
            return write_result(&mut self.output, id, Value::Null);
        };
        let original = document.text.clone();
        self.next_request += 1;
        let request_id = json!(format!("papyrus-lint-{}", self.next_request));
        let body = serde_json::to_vec(&json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "workspace/applyEdit",
            "params": {
                "label": "Papyrus Lint: fix file",
                "edit": code_actions::replace_edit(&uri, &original, &repaired),
            }
        }))
        .expect("applyEdit json");
        write_message(&mut self.output, &body)?;
        let response = self.read_response(&request_id)?;
        if response.get("error").is_some() || response["result"]["applied"] == false {
            return write_error(&mut self.output, id, REQUEST_FAILED, "edit was not applied");
        }
        self.documents
            .replace_text(&uri, repaired, &mut self.output)?;
        write_result(&mut self.output, id, Value::Null)
    }

    fn read_response(&mut self, request_id: &Value) -> io::Result<Value> {
        loop {
            let Some(message) = self.read_message()? else {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "eof waiting for workspace/applyEdit",
                ));
            };
            if message.get("method").is_none() && message.get("id") == Some(request_id) {
                return Ok(message);
            }
            if let Some(code) = self.dispatch(message)? {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("client exited ({code}) while applying an edit"),
                ));
            }
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
