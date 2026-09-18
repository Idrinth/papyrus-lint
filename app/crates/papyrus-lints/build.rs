//! Generates the lint crate's static data, configuration, dispatch code,
//! and rule `mod` declarations.

mod build_support;

use build_support::{data_tables, dispatch, metadata, BuildContext};

fn main() {
    let context = BuildContext::from_env();
    let rules = metadata::load(&context);

    metadata::validate(&rules).unwrap_or_else(|error| panic!("{error}"));
    data_tables::compile(&context, &rules);
    dispatch::compile(&context, &rules);
}
