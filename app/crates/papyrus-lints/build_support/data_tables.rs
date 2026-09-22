use super::metadata::RuleMetadata;
use super::renderer::Renderer;
use super::{generated_header, BuildContext};
use serde::Deserialize;

#[derive(Deserialize)]
struct ForbiddenRule {
    script: String,
    function: String,
    level: String,
    message: String,
    #[serde(default)]
    global: bool,
}
#[derive(Deserialize)]
struct DeprecatedRule {
    script: String,
    function: String,
    replacement: Option<String>,
    message: String,
    #[serde(default)]
    global: bool,
}
#[derive(Deserialize)]
struct SlowRule {
    object: String,
    function: String,
    replacement: String,
    #[serde(default)]
    global: bool,
}
#[derive(Deserialize)]
struct NativeMethod {
    object: String,
    function: String,
}
#[derive(Deserialize)]
struct UpdateEventPair {
    register: String,
    event: String,
}
#[derive(Deserialize)]
struct EventArg {
    #[serde(rename = "type")]
    type_name: String,
    name: String,
}
#[derive(Deserialize)]
struct KnownEvent {
    event: String,
    form: String,
    args: Vec<EventArg>,
}

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
    let values: Vec<DeprecatedRule> = context.load_yaml(
        "shared/rules/data/skyrim/deprecated-functions.yaml",
        "deprecated-functions rules",
    );
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/skyrim/deprecated-functions.yaml",
    ));
    out.line("pub static DEPRECATED_FUNCTIONS: &[DeprecatedFunctionRule] = &[");
    for rule in values {
        out.line(format_args!("    DeprecatedFunctionRule {{ script: {:?}, function: {:?}, replacement: {:?}, message: {:?}, global: {:?} }},", rule.script, rule.function, rule.replacement, rule.message, rule.global));
    }
    out.line("];");
    context.write("deprecated_functions_data.rs", "rule data", &out.finish());
}

fn table<T>(
    context: &BuildContext,
    input: &str,
    description: &str,
    output: &str,
    declaration: &str,
    render: impl Fn(&T) -> String,
) where
    T: serde::de::DeserializeOwned,
{
    let values: Vec<T> = context.load_yaml(input, description);
    let mut out = Renderer::new();
    out.line(generated_header(input));
    out.line(declaration);
    for value in &values {
        out.line(render(value));
    }
    out.line("];");
    context.write(output, "rule data", &out.finish());
}

fn forbidden_functions(context: &BuildContext) {
    let values: Vec<ForbiddenRule> = context.load_yaml(
        "shared/rules/data/skyrim/forbidden-functions.yaml",
        "forbidden-functions rules",
    );
    let mut out = Renderer::new();
    out.line(generated_header(
        "shared/rules/data/skyrim/forbidden-functions.yaml",
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
    table(
        context,
        "shared/rules/data/skyrim/slow-functions.yaml",
        "slow-functions rules",
        "slow_functions_data.rs",
        "pub static SLOW_FUNCTIONS: &[SlowFunctionRule] = &[",
        |r: &SlowRule| {
            format!("    SlowFunctionRule {{ object: {:?}, function: {:?}, replacement: {:?}, global: {:?} }},", r.object, r.function, r.replacement, r.global)
        },
    );
}
fn native_methods(context: &BuildContext) {
    table(
        context,
        "shared/rules/data/skyrim/native-methods.yaml",
        "native-methods rules",
        "native_methods_data.rs",
        "pub static NATIVE_METHODS: &[NativeMethodRule] = &[",
        |r: &NativeMethod| {
            format!(
                "    NativeMethodRule {{ object: {:?}, function: {:?} }},",
                r.object, r.function
            )
        },
    );
}
fn actor_values(context: &BuildContext) {
    table(
        context,
        "shared/rules/data/skyrim/actor-values.yaml",
        "actor-values rules",
        "actor_values_data.rs",
        "pub static ACTOR_VALUES: &[&str] = &[",
        |value: &String| format!("    {value:?},"),
    );
}
fn update_event_pairs(context: &BuildContext) {
    table(
        context,
        "shared/rules/data/skyrim/update-event-handlers.yaml",
        "update-event-handlers rules",
        "update_event_pairs_data.rs",
        "pub static UPDATE_EVENT_PAIRS: &[UpdateEventPairRule] = &[",
        |r: &UpdateEventPair| {
            format!(
                "    UpdateEventPairRule {{ register: {:?}, event: {:?} }},",
                r.register, r.event
            )
        },
    );
}

fn known_events(context: &BuildContext) {
    table(
        context,
        "shared/rules/data/skyrim/known-events.yaml",
        "known-events rules",
        "known_events_data.rs",
        "pub static KNOWN_EVENTS: &[KnownEventRule] = &[",
        |event: &KnownEvent| {
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
                "    KnownEventRule {{ event: {:?}, form: {:?}, args: &[{args}] }},",
                event.event, event.form
            )
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
