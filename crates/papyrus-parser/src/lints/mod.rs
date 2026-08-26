//! Lint rules built on top of the Papyrus AST.
//!
//! Each lint takes a parsed `Script` and returns a list of findings; none
//! of them re-parse or re-lex the source.

pub mod forbidden_functions;
