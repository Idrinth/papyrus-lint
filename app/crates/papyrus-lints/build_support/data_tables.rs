use super::metadata::RuleMetadata;
use super::policy;
use super::renderer::Renderer;
use super::script_catalog;
use super::{generated_header, BuildContext};

pub fn compile(context: &BuildContext, rules: &[RuleMetadata]) {
    forbidden_functions(context);
    deprecated_functions(context);
    slow_functions(context);
    native_methods(context);
    actor_values(context);
    update_event_pairs(context);
    known_events(context);
    rule_tags(context, rules);
    known_rule_ids(context, rules);
}

struct GameTableSpec<'a> {
    filename: &'a str,
    header: &'a str,
    item_ty: &'a str,
    const_name: &'a str,
    selector: &'a str,
    skyrim_rows: &'a [String],
    fallout4_rows: &'a [String],
}

fn emit_game_tables(context: &BuildContext, spec: GameTableSpec<'_>) {
    let GameTableSpec {
        filename,
        header,
        item_ty,
        const_name,
        selector,
        skyrim_rows,
        fallout4_rows,
    } = spec;
    let mut out = Renderer::new();
    out.line(generated_header(header));
    emit_static(
        &mut out,
        &format!("SKYRIM_{const_name}"),
        item_ty,
        skyrim_rows,
    );
    out.blank();
    emit_static(
        &mut out,
        &format!("FALLOUT4_{const_name}"),
        item_ty,
        fallout4_rows,
    );
    out.blank();
    out.line(format!(
        "pub static {const_name}: &[{item_ty}] = SKYRIM_{const_name};"
    ));
    out.blank();
    out.block(
        format!("pub fn {selector}(game: &str) -> &'static [{item_ty}]"),
        |out| {
            out.block("match game", |out| {
                out.line(format!("\"fallout4\" => FALLOUT4_{const_name},"));
                out.line(format!("_ => SKYRIM_{const_name},"));
            });
        },
    );
    context.write(filename, "rule data", &out.finish());
}

fn emit_static(out: &mut Renderer, name: &str, item_ty: &str, rows: &[String]) {
    out.line(format!("pub static {name}: &[{item_ty}] = &["));
    for row in rows {
        out.line(format!("    {row}"));
    }
    out.line("];");
}

fn deprecated_functions(context: &BuildContext) {
    let row = |rule: &policy::DeprecatedFunction| {
        format!(
            "DeprecatedFunctionRule {{ script: {:?}, function: {:?}, replacement: {:?}, message: {:?}, global: {:?} }},",
            rule.script, rule.function, rule.replacement, rule.message, rule.global
        )
    };
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "deprecated_functions_data.rs",
            header: "shared/rules/data/{skyrim,fallout4}/deprecated-functions.yaml",
            item_ty: "DeprecatedFunctionRule",
            const_name: "DEPRECATED_FUNCTIONS",
            selector: "deprecated_functions_for",
            skyrim_rows: &policy::deprecated_functions(context, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &policy::deprecated_functions(context, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn forbidden_functions(context: &BuildContext) {
    let row = |rule: &policy::ForbiddenFunction| {
        if !matches!(rule.level.as_str(), "error" | "warning" | "info") {
            panic!(
                "forbidden-functions.yaml: unknown level `{}` for {}.{}",
                rule.level, rule.script, rule.function
            );
        }
        format!(
            "ForbiddenFunctionRule {{ script: {:?}, function: {:?}, level: {:?}, message: {:?}, global: {:?} }},",
            rule.script, rule.function, rule.level, rule.message, rule.global
        )
    };
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "forbidden_functions_data.rs",
            header: "shared/rules/data/{skyrim,fallout4}/forbidden-functions.yaml",
            item_ty: "ForbiddenFunctionRule",
            const_name: "FORBIDDEN_FUNCTIONS",
            selector: "forbidden_functions_for",
            skyrim_rows: &policy::forbidden_functions(context, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &policy::forbidden_functions(context, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn slow_functions(context: &BuildContext) {
    let row = |rule: &policy::SlowFunction| {
        format!(
            "SlowFunctionRule {{ object: {:?}, function: {:?}, replacement: {:?}, global: {:?} }},",
            rule.object, rule.function, rule.replacement, rule.global
        )
    };
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "slow_functions_data.rs",
            header: "shared/rules/data/{skyrim,fallout4}/slow-functions.yaml",
            item_ty: "SlowFunctionRule",
            const_name: "SLOW_FUNCTIONS",
            selector: "slow_functions_for",
            skyrim_rows: &policy::slow_functions(context, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &policy::slow_functions(context, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn native_methods(context: &BuildContext) {
    let scripts_dir = context.input("shared/scripts");
    let row = |rule: &script_catalog::NativeMethod| {
        format!(
            "NativeMethodRule {{ object: {:?}, function: {:?} }},",
            rule.object, rule.function
        )
    };
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "native_methods_data.rs",
            header: "bundled Creation Kit archives under shared/scripts",
            item_ty: "NativeMethodRule",
            const_name: "NATIVE_METHODS",
            selector: "native_methods_for",
            skyrim_rows: &script_catalog::native_methods(&scripts_dir, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &script_catalog::native_methods(&scripts_dir, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn actor_values(context: &BuildContext) {
    let row = |value: &String| format!("{value:?},");
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "actor_values_data.rs",
            header: "shared/rules/data/{skyrim,fallout4}/actor-values.yaml",
            item_ty: "&str",
            const_name: "ACTOR_VALUES",
            selector: "actor_values_for",
            skyrim_rows: &policy::actor_values(context, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &policy::actor_values(context, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn update_event_pairs(context: &BuildContext) {
    let row = |rule: &policy::UpdateEventPair| {
        format!(
            "UpdateEventPairRule {{ register: {:?}, event: {:?} }},",
            rule.register, rule.event
        )
    };
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "update_event_pairs_data.rs",
            header: "shared/rules/data/{skyrim,fallout4}/update-event-handlers.yaml",
            item_ty: "UpdateEventPairRule",
            const_name: "UPDATE_EVENT_PAIRS",
            selector: "update_event_pairs_for",
            skyrim_rows: &policy::update_event_pairs(context, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &policy::update_event_pairs(context, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn known_events(context: &BuildContext) {
    let row = |event: &script_catalog::KnownEvent| {
        let args = event
            .args
            .iter()
            .map(|arg| {
                format!(
                    "EventArg {{ type_name: {:?}, name: {:?} }}, ",
                    arg.type_name, arg.name
                )
            })
            .collect::<String>();
        format!(
            "KnownEventRule {{ event: {:?}, form: {:?}, args: &[{args}] }},",
            event.event, event.form
        )
    };
    let scripts_dir = context.input("shared/scripts");
    emit_game_tables(
        context,
        GameTableSpec {
            filename: "known_events_data.rs",
            header: "bundled Creation Kit archives under shared/scripts",
            item_ty: "KnownEventRule",
            const_name: "KNOWN_EVENTS",
            selector: "known_events_for",
            skyrim_rows: &script_catalog::known_events(&scripts_dir, "skyrim")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
            fallout4_rows: &script_catalog::known_events(&scripts_dir, "fallout4")
                .iter()
                .map(row)
                .collect::<Vec<_>>(),
        },
    );
}

fn rule_tags(context: &BuildContext, rules: &[RuleMetadata]) {
    let mut out = Renderer::new();
    out.line(generated_header("shared/rules.json"));
    out.line("pub static RULE_TAGS: &[RuleTags] = &[");
    for rule in rules {
        let importance = match rule.importance.as_str() {
            "low" => "Importance::Low",
            "medium" => "Importance::Medium",
            "high" => "Importance::High",
            other => panic!(
                "shared/rules.json: unknown importance `{other}` for {}",
                rule.id
            ),
        };
        let tags = rule
            .tags
            .iter()
            .map(|tag| format!("{tag:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.line(format_args!("    RuleTags {{ rule: {:?}, description: {:?}, kinds: &[{tags}], importance: {importance} }},", rule.id, rule.definition));
    }
    out.line("];");
    context.write("rule_tags_data.rs", "rule data", &out.finish());
}

fn known_rule_ids(context: &BuildContext, rules: &[RuleMetadata]) {
    let mut out = Renderer::new();
    out.line(generated_header("shared/rules.json"));
    out.line("pub const KNOWN_RULE_IDS: &[&str] = &[");
    for rule in rules {
        out.line(format_args!("    {:?},", rule.id));
    }
    out.line("];");
    out.blank();
    out.line(generated_header("shared/rules.json"));
    out.line("pub const FIXABLE_RULE_IDS: &[&str] = &[");
    for rule in rules.iter().filter(|rule| rule.fixable) {
        out.line(format_args!("    {:?},", rule.id));
    }
    out.line("];");
    context.write("known_rule_ids_data.rs", "rule data", &out.finish());
}
