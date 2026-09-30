//! Generates the lint crate's static data, configuration, dispatch code,
//! and rule `mod` declarations.

mod build_support;

use build_support::{data_tables, dispatch, lint_settings, metadata, BuildContext};

fn main() {
    let context = BuildContext::from_env();
    lint_settings::compile(&context);
    let rules = metadata::load(&context);
    let rule_order = metadata::load_rule_order(&context);

    metadata::validate(&rules, &rule_order.repair, &rule_order.external_repair)
        .unwrap_or_else(|error| panic!("{error}"));
    data_tables::compile(&context, &rules);
    dispatch::compile(&context, &rules, &rule_order.repair);
}
