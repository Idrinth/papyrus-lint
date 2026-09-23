//! Fallout 4's Papyrus dialect: custom `Struct`s, property `Group`s,
//! Fallout-specific declaration flags, colon-qualified names, the `is`
//! type-check operator, and remote / custom events
//! (`Event OtherScript.EventName(...)`). All are opt-in through
//! [`GameEdition::Fallout4`]. Rejection of the same constructs in Skyrim
//! mode lives in `skyrim_mode.rs`.

use papyrus_parser::ast::Expr;
use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse_with_mode, PapyrusError};
