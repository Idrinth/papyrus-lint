//! Generates `Config` (every field except [`Rules`]) from
//! `shared/configuration/lint-settings.yaml`.

use super::renderer::Renderer;
use super::BuildContext;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const RUST_TYPES: &[&str] = &[
    "bool",
    "usize",
    "f64",
    "Game",
    "Indentation",
    "IdentifierCasing",
    "TypeCasing",
    "NamedArguments",
    "MagicNumbers",
    "Hungarian",
    "EncodingEnforced",
];
