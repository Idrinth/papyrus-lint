//! Black-box tests for the crate-level lint and repair entry points.

use papyrus_lints::{
    lint, lint_with_external_arguments, repair, repair_filtered, repair_filtered_by_tag,
    restrict_to_line, tags::tags_for, Config, Diagnostic, ExternalSignatures, NamedArguments,
    ParamInfo, FIXABLE_RULE_IDS, KNOWN_RULE_IDS,
};
use papyrus_parser::ast::TypeName;
use std::collections::HashSet;
