//! Injects the README-synced explanatory comments above each top-level key
//! of a saved papyrus-lint YAML document (see
//! [`crate::project_file::save_config`]/[`crate::project_file::save_config_at_path`]
//! and [`crate::presets::save_user_preset`]).

/// The explanatory comment shown above each top-level key in the README's
/// [configuration reference](../../../../README.md#configuration), in the
/// same order `ProjectFile`/`papyrus_lints::Config` declare their fields.
/// Kept in sync with that table so a saved config file documents itself
/// the same way.
const FIELD_COMMENTS: &[(&str, &str)] = &[
    (
        "compiler_path",
        "# Path to PapyrusCompiler.exe, or null to auto-detect it",
    ),
    (
        "additional_script_roots",
        "# Extra directories (relative to the project root, or absolute) to search\n\
         # for .psc files, besides scripts/source and source/scripts",
    ),
    (
        "lookup_script_roots",
        "# Extra directories searched only as a fallback when resolving other\n\
         # scripts for analysis (argument/return types, Extends, autocompletion).\n\
         # Scripts found only here are never linted, and these directories are\n\
         # ignored by conflicting-script-versions. Creating or updating a config\n\
         # fills Skyrim Special Edition's Data/Scripts/Source and Data/Source/Scripts\n\
         # when the install path can be read from the Windows registry",
    ),
    (
        "compile_check",
        "# true, false; also runs PapyrusCompiler.exe (into a throwaway temporary\n\
         # directory) as part of linting a dropped .psc, reporting its errors\n\
         # alongside the lint engine's own. Requires compiler_path to be set/\n\
         # auto-detected",
    ),
    (
        "strict_achlist_scope",
        "# true enables strict cross-script resolution and conflicting-script-versions\n\
         # checks for only the achlist's listed entries. false (the default) keeps\n\
         # parent-directory search roots. In strict mode, every .psc dependency must\n\
         # be listed in the achlist",
    ),
    ("semicolon", "# true, false"),
    ("indentation", "# tab, space"),
    (
        "indentation_width",
        "# Non-negative integer; used only when indentation is space",
    ),
    (
        "identifier_casing",
        "# camelCase, PascalCase, snake_case, CONSTANT_CASE",
    ),
    ("cyclomatic_complexity_warning", "# Non-negative integer"),
    ("cyclomatic_complexity_error", "# Non-negative integer"),
    (
        "type_casing",
        "# PascalCase, camelCase, lowercase, UPPERCASE",
    ),
    ("named_arguments", "# always, instead_of_defaults, never"),
    ("min_wait_interval", "# Non-negative number"),
    ("magic_numbers", "# loose, strict"),
    ("fail_on_warning", "# true, false"),
    ("fail_on_info", "# true, false"),
    ("bool_like_int", "# true, false"),
    (
        "assume_auto_properties_filled",
        "# true, false; true treats an Auto/AutoReadOnly property as already\n\
         # filled in by the time a function runs instead of possibly None",
    ),
    ("rules", "# Each rule accepts true or false"),
];

/// Inserts [`FIELD_COMMENTS`] above their matching top-level key in `yaml`.
/// Only unindented `key:` lines are matched, so the `rules:` block's own
/// nested keys are left alone, matching the single comment the README
/// shows above `rules:` itself rather than one per rule.
pub(crate) fn with_field_comments(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + FIELD_COMMENTS.len() * 32);
    for line in yaml.lines() {
        if !line.starts_with(' ') {
            if let Some(key) = line.split(':').next() {
                if let Some((_, comment)) = FIELD_COMMENTS.iter().find(|(name, _)| *name == key) {
                    out.push_str(comment);
                    out.push('\n');
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
