use papyrus_lints::{Config, Diagnostic};

/// The result of linting one in-memory source buffer.
///
/// `diagnostics` are the engine findings for `source` under the supplied
/// configuration. `parser_failure` is the first lexer or parser error, if
/// the buffer is not valid Papyrus — reported separately so a caller can
/// tell "this is not a script" apart from "this is a script and a rule
/// fired". Currently at most one failure, because [`papyrus_parser::parse`]
/// stops at the first error.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveAnalysis {
    pub diagnostics: Vec<Diagnostic>,
    pub parser_failure: Option<ParserFailure>,
}

impl LiveAnalysis {
    /// Whether the buffer failed to lex or parse.
    pub fn parse_failed(&self) -> bool {
        self.parser_failure.is_some()
    }
}

/// Whether a [`ParserFailure`] came from the lexer or the recursive-descent
/// parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserFailureKind {
    Lex,
    Parse,
}

/// One lexer or parser error raised while handling a live buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserFailure {
    pub kind: ParserFailureKind,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Lints `source` as an in-memory Papyrus buffer under `config`.
///
/// This is the shared body of `PapyrusLinterCLI --blob` and the LSP
/// document snapshot: call [`papyrus_lints::lint`] and collect the first
/// parse failure, without any project-level machinery.
pub fn lint_source(source: &str, config: &Config) -> LiveAnalysis {
    LiveAnalysis {
        diagnostics: papyrus_lints::lint(source, config),
        parser_failure: parser_failure(source),
    }
}

fn parser_failure(source: &str) -> Option<ParserFailure> {
    match papyrus_parser::parse(source) {
        Ok(_) => None,
        Err(papyrus_parser::PapyrusError::Lex(error)) => Some(ParserFailure {
            kind: ParserFailureKind::Lex,
            line: error.line,
            column: error.col,
            message: error.message,
        }),
        Err(papyrus_parser::PapyrusError::Parse(error)) => Some(ParserFailure {
            kind: ParserFailureKind::Parse,
            line: error.line,
            column: error.col,
            message: error.message,
        }),
    }
}
