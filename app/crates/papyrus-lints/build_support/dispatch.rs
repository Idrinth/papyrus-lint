use super::metadata::{self, RuleMetadata, NO_SOURCE_CHECK_IDS};
use super::renderer::Renderer;
use super::{default_config_order, BuildContext};
use std::collections::BTreeSet;

pub fn compile(context: &BuildContext, rules: &[RuleMetadata]) {
    let relative = "configuration/papyrus-lint.default.yaml";
    let source = context.load_text(relative, "default config");
    let order = default_config_order(&source, &context.input(relative))
        .unwrap_or_else(|error| panic!("{error}"));
    let ordered =
        metadata::order_by_config(rules, &order).unwrap_or_else(|error| panic!("{error}"));
    rules_struct(context, &ordered);
    rules_dispatch(context, rules);
}

fn rules_struct(context: &BuildContext, rules: &[&RuleMetadata]) {
    let mut out = Renderer::new();
    out.line("/// Individual enable/disable switches for each lint ruleset.");
    out.line("/// Generated from `shared/rules.json` by `build.rs`. Do not edit by hand.");
    out.line("///");
    out.line("/// A ruleset set to `false` here is skipped by both");
    out.line("/// [`crate::lint`]/[`crate::lint_with_external_arguments`] and, for");
    out.line("/// rulesets with an automatic fix, [`crate::repair`]. Most rulesets");
    out.line("/// default to `true`; those tagged `enabled_by_default: false` in");
    out.line("/// `shared/rules.json` default to `false`.");
    out.line("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]");
    out.line("#[serde(default)]");
    out.block("pub struct Rules", |out| {
        for rule in rules {
            let kind = if rule.fixable { "lint/fix" } else { "lint" };
            out.line(format_args!("/// The {:?} {kind}.", rule.name));
            if !rule.enabled_by_default {
                out.line("/// Defaults to `false`.");
            }
            out.line(format_args!(
                "pub {}: bool,",
                metadata::config_key(&rule.id)
            ));
        }
    });
    out.blank();
    out.line("/// Default enable/disable flags for [`Rules`]. Generated from");
    out.line("/// `shared/rules.json` (`enabled_by_default`, defaulting to `true`).");
    out.block("pub fn default_rules() -> Rules", |out| {
        out.block("Rules", |out| {
            for rule in rules {
                out.line(format_args!(
                    "{}: {},",
                    metadata::config_key(&rule.id),
                    rule.enabled_by_default
                ));
            }
        });
    });
    context.write("rules_struct.rs", "Rules struct", &out.finish());
}

fn rules_dispatch(context: &BuildContext, rules: &[RuleMetadata]) {
    let modules: BTreeSet<_> = rules
        .iter()
        .filter(|rule| !NO_SOURCE_CHECK_IDS.contains(&rule.id.as_str()))
        .map(|rule| metadata::module_name(&rule.id))
        .collect();
    let mut out = Renderer::new();
    out.line("use crate::{");
    out.line(format_args!(
        "    {}, Diagnostic,",
        modules.into_iter().collect::<Vec<_>>().join(", ")
    ));
    out.line("};");
    out.blank();
    out.line("/// Runs every enabled source-level lint against `source`.");
    out.line("/// Generated from `shared/rules.json` by `build.rs`. Do not edit by hand.");
    out.line("#[allow(clippy::too_many_lines)]");
    out.line("pub fn collect_diagnostics<E: ExternalSignatures>(");
    out.line("    source: &str,");
    out.line("    config: &Config,");
    out.line("    external: &mut E,");
    out.line(") -> Vec<Diagnostic> {");
    for line in [
        "let tokens = papyrus_parser::tokenize(source).ok();",
        "let tokens = tokens.as_deref();",
        "let ast = papyrus_parser::parse(source).ok();",
        "let ast = ast.as_ref();",
        "let rules = &config.rules;",
        "let mut diagnostics = Vec::new();",
    ] {
        out.line(format_args!("    {line}"));
    }
    for rule in rules
        .iter()
        .filter(|rule| !NO_SOURCE_CHECK_IDS.contains(&rule.id.as_str()))
    {
        let key = metadata::config_key(&rule.id);
        let module = metadata::module_name(&rule.id);
        out.line(format_args!("    if rules.{key} {{"));
        out.line(format_args!(
            "        diagnostics.extend({module}::check(source, ast, tokens, config, external));"
        ));
        out.line("    }");
    }
    out.line("    diagnostics");
    out.line("}");
    out.blank();
    out.line("/// Applies every self-contained automatic fix whose ruleset is enabled.");
    out.line("/// Generated from `shared/rules.json` by `build.rs`. Do not edit by hand.");
    out.line("/// Repair order is `repair_order` in that file (not rule-id order),");
    out.line("/// because later fixes see earlier rewrites.");
    out.line("#[allow(clippy::too_many_lines)]");
    out.line("pub fn apply_repairs(source: &str, config: &Config, applies: impl Fn(&str) -> bool) -> String {");
    out.line("    let rules = &config.rules;");
    out.line("    let mut source = source.to_string();");
    let mut repairs: Vec<_> = rules
        .iter()
        .filter(|rule| rule.repair_order.is_some())
        .collect();
    repairs.sort_by_key(|rule| rule.repair_order);
    for rule in repairs {
        let key = metadata::config_key(&rule.id);
        let module = metadata::module_name(&rule.id);
        for line in [
            "    source = apply_rule(".to_string(),
            "        source,".to_string(),
            format!("        rules.{key} && applies({module}::RULE),"),
            "        |source| {".to_string(),
            "            let tokens = papyrus_parser::tokenize(source).ok();".to_string(),
            "            let ast = papyrus_parser::parse(source).ok();".to_string(),
            format!(
                "            {module}::repair(source, ast.as_ref(), tokens.as_deref(), config)"
            ),
            "        },".to_string(),
            "    );".to_string(),
        ] {
            out.line(line);
        }
    }
    out.line("    source");
    out.line("}");
    context.write("rules_dispatch.rs", "dispatch", &out.finish());
}
