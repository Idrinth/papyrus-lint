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

fn deprecated_functions(context: &BuildContext) {
    let values = policy::deprecated_functions(context);
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/{skyrim,fallout4}/deprecated-functions.yaml",
    ));
    out.line("pub static DEPRECATED_FUNCTIONS: &[DeprecatedFunctionRule] = &[");
    for rule in values {
        out.line(format_args!("    DeprecatedFunctionRule {{ script: {:?}, function: {:?}, replacement: {:?}, message: {:?}, global: {:?} }},", rule.script, rule.function, rule.replacement, rule.message, rule.global));
    }
    out.line("];");
    context.write("deprecated_functions_data.rs", "rule data", &out.finish());
}

fn forbidden_functions(context: &BuildContext) {
    let values = policy::forbidden_functions(context);
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/{skyrim,fallout4}/forbidden-functions.yaml",
    ));
    out.line("pub static FORBIDDEN_FUNCTIONS: &[ForbiddenFunctionRule] = &[");
    for rule in values {
        if !matches!(rule.level.as_str(), "error" | "warning" | "info") {
            panic!(
                "forbidden-functions.yaml: unknown level `{}` for {}.{}",
                rule.level, rule.script, rule.function
            );
        }
        out.line(format_args!("    ForbiddenFunctionRule {{ script: {:?}, function: {:?}, level: {:?}, message: {:?}, global: {:?} }},", rule.script, rule.function, rule.level, rule.message, rule.global));
    }
    out.line("];");
    context.write("forbidden_functions_data.rs", "rule data", &out.finish());
}

fn slow_functions(context: &BuildContext) {
    let values = policy::slow_functions(context);
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/{skyrim,fallout4}/slow-functions.yaml",
    ));
    out.line("pub static SLOW_FUNCTIONS: &[SlowFunctionRule] = &[");
    for rule in values {
        out.line(format_args!("    SlowFunctionRule {{ object: {:?}, function: {:?}, replacement: {:?}, global: {:?} }},", rule.object, rule.function, rule.replacement, rule.global));
    }
    out.line("];");
    context.write("slow_functions_data.rs", "rule data", &out.finish());
}

fn native_methods(context: &BuildContext) {
    let scripts_dir = context.input("shared/scripts");
    let values = script_catalog::native_methods(&scripts_dir);
    let mut out = Renderer::new();
    out.line(generated_header(
        "bundled Creation Kit archives under shared/scripts",
    ));
    out.line("pub static NATIVE_METHODS: &[NativeMethodRule] = &[");
    for rule in values {
        out.line(format_args!(
            "    NativeMethodRule {{ object: {:?}, function: {:?} }},",
            rule.object, rule.function
        ));
    }
    out.line("];");
    context.write("native_methods_data.rs", "rule data", &out.finish());
}

fn actor_values(context: &BuildContext) {
    let values = policy::actor_values(context);
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/{skyrim,fallout4}/actor-values.yaml",
    ));
    out.line("pub static ACTOR_VALUES: &[&str] = &[");
    for value in values {
        out.line(format_args!("    {value:?},"));
    }
    out.line("];");
    context.write("actor_values_data.rs", "rule data", &out.finish());
}

fn update_event_pairs(context: &BuildContext) {
    let values = policy::update_event_pairs(context);
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/{skyrim,fallout4}/update-event-handlers.yaml",
    ));
    out.line("pub static UPDATE_EVENT_PAIRS: &[UpdateEventPairRule] = &[");
    for rule in values {
        out.line(format_args!(
            "    UpdateEventPairRule {{ register: {:?}, event: {:?} }},",
            rule.register, rule.event
        ));
    }
    out.line("];");
    context.write("update_event_pairs_data.rs", "rule data", &out.finish());
}

fn known_events(context: &BuildContext) {
    let scripts_dir = context.input("shared/scripts");
    let values = script_catalog::known_events(&scripts_dir);
    let mut out = Renderer::new();
    out.line(generated_header(
        "bundled Creation Kit archives under shared/scripts",
    ));
    out.line("pub static KNOWN_EVENTS: &[KnownEventRule] = &[");
    for event in values {
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
        out.line(format_args!(
            "    KnownEventRule {{ event: {:?}, form: {:?}, args: &[{args}] }},",
            event.event, event.form
        ));
    }
    out.line("];");
    context.write("known_events_data.rs", "rule data", &out.finish());
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
