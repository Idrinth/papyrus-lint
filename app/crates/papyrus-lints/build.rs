//! Generates the lint crate's static data, configuration, dispatch code,
//! and rule `mod` declarations.

mod build_support;

use build_support::{data_tables, dispatch, lint_settings, metadata, BuildContext};

fn main() {
    let context = BuildContext::from_env();
    lint_settings::compile(&context);
    let rules = metadata::load(&context);
    let repair_order = metadata::load_repair_order(&context);

    metadata::validate(&rules, &repair_order).unwrap_or_else(|error| panic!("{error}"));
    data_tables::compile(&context, &rules);
    dispatch::compile(&context, &rules, &repair_order);
}
