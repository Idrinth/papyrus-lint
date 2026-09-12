//! Metadata tags for every rule in [`crate::KNOWN_RULE_IDS`]: the kind(s) of
//! fix its findings represent (e.g. `"style"`, `"performance"`,
//! `"correctness"`, `"maintainability"`), how important addressing them is
//! to keeping a codebase maintainable, whether they're auto-fixable, and a
//! detailed description of the rule (copied from its row in README.md's
//! Implemented Lints tables, kept in sync by hand the same way
//! `docs/nexuspage.bbcode`'s own lint descriptions are — see "Keeping agent
//! instructions synchronized" in CLAUDE.md/AGENTS.md). This module only
//! exposes that metadata; [`crate::repair_filtered_by_tag`] and the CLI's
//! `--tag <kind>` flag are what actually filter lints/fixes down to one
//! kind at a time, built on top of it.

use crate::FIXABLE_RULE_IDS;

/// How important fixing a rule's findings is to keeping a codebase
/// maintainable over time. Independent of the `[error]`/`[warning]`/`[info]`
/// level(s) a rule's own diagnostics carry, which instead reflect runtime
/// risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Importance {
    Low,
    Medium,
    High,
}

/// Tags describing one rule, keyed by its [`Diagnostic::rule`](crate::Diagnostic::rule) id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleTags {
    pub rule: &'static str,
    /// The rule's detailed description, copied verbatim from its row in
    /// README.md's Implemented Lints tables, so a consumer (e.g. the
    /// desktop app's "Export for AI" document) can surface the same
    /// explanation the README gives a human reader without needing that
    /// documentation on hand.
    pub description: &'static str,
    /// Keyword(s) describing the kind(s) of fix this rule's findings
    /// represent. Never empty.
    pub kinds: &'static [&'static str],
    pub importance: Importance,
}

impl RuleTags {
    /// Whether this rule has an automatic fix. Derived from
    /// [`crate::FIXABLE_RULE_IDS`] rather than stored on each entry here, so
    /// the two can never drift out of sync.
    pub fn auto_fixable(&self) -> bool {
        FIXABLE_RULE_IDS.contains(&self.rule)
    }
}

/// Looks up a rule's [`RuleTags`] by its id, matched case-insensitively (the
/// same convention `@disable` directives are matched against — see
/// `disable_comments`). Returns `None` for an id not in
/// [`crate::KNOWN_RULE_IDS`].
pub fn tags_for(rule: &str) -> Option<&'static RuleTags> {
    RULE_TAGS
        .iter()
        .find(|tags| tags.rule.eq_ignore_ascii_case(rule))
}

/// One entry per id in [`crate::KNOWN_RULE_IDS`], in the same order.
pub const RULE_TAGS: &[RuleTags] = &[
    RuleTags {
        rule: crate::trailing_whitespace::RULE,
        description: "Flags, as a `[warning]`, lines that end with trailing spaces or tabs.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::comma_spacing::RULE,
        description: "Requires, as a `[warning]`, whitespace after commas in argument lists.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::forbidden_functions::RULE,
        description: "Flags calls to functions listed in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls), with a configurable severity and an explanatory message per entry.",
        kinds: &["performance", "correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::formid_hex_notation::RULE,
        description: "Flags, as a `[warning]`, a FormID literal that isn't written in hexadecimal notation when it's directly compared (`==`, `!=`, `<`, `<=`, `>`, `>=`) against a `GetFormID()` call, or passed as the FormID argument to `Game.GetFormFromFile` (positionally or by name), since hexadecimal is the convention used everywhere else a FormID appears (the Creation Kit, xEdit, mod documentation) and a stray decimal literal is easy to mistype or overlook. Only a literal directly adjacent to the comparison operator or the call's argument list is checked; one reached indirectly through a variable assigned earlier is left unflagged rather than guessed at.",
        kinds: &["correctness", "style"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::slow_functions::RULE,
        description: "Flags calls to functions listed in `rules/slow-functions.yaml` that have a faster equivalent available, and suggests the quicker alternative. The fix replaces the complete call with that rule's supplied replacement, preserving the original argument where the replacement uses the `value` placeholder.",
        kinds: &["performance"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_getter::RULE,
        description: "Flags calls to functions whose names begin with `Get` (case-insensitively) whose result is discarded, whether the call stands alone (`GetValue()`) or only feeds a comparison, arithmetic, or logical operator whose own result is then discarded too (e.g. `GetDistance(target) > 0` on its own line, with no assignment, `Return`, or condition around it).",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_property::RULE,
        description: "Flags `Property` declarations whose name is never referenced anywhere else in the script.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::semicolon::RULE,
        description: "Requires, as a `[warning]`, a trailing semicolon on each non-empty line or forbids terminal semicolons, according to the selected setting.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::float_int_conversion::RULE,
        description: "Flags a Float value declared, assigned, returned, or passed as an argument into an Int-typed slot without an explicit `as Int` cast.",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::int_division_to_float::RULE,
        description: "Flags an `Int / Int` division declared, assigned, returned, or passed as an argument into a Float-typed slot without either operand already being a Float, since Papyrus performs the division as integer division — truncating towards zero — before the result ever widens into the Float slot (e.g. `Float f = 1 / 2` yields `0.0`, not `0.5`). Casting the division's *result* to Float doesn't avoid this, only casting (or writing) an *operand* as a Float does, so only the latter is left unflagged. When both operands are integer literals (optionally combined with `+`/`-`/`*`/unary `-`) and the division happens to divide evenly (e.g. `Float f = 72 / 8`), no truncation actually occurs, so that case is left unflagged too.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::strict_boolean::RULE,
        description: "Flags `If`/`ElseIf`/`While` conditions that aren't already a `Bool` value or expression, instead of relying on Papyrus's implicit conversion to boolean. Only conditions whose type can be determined locally (locals, parameters, properties, literals, casts, and comparison/logical expressions) are checked; a condition that depends on a function call or a member access is left unflagged rather than risk a false positive. By default (`bool_like_int: true`), the `Int` literal `1` or `0` used directly as a condition is allowed as a common \"bool-like\" idiom; any other `Int` value (including a variable or property that happens to hold `0`/`1`) is still flagged, and setting `bool_like_int: false` flags the literals too.",
        kinds: &["correctness", "style"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::argument_types::RULE,
        description: "Flags call-site arguments whose type doesn't match the callee's declared parameter type (e.g. passing a `String` where an `Int` is expected), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as passing an object whose script extends (directly or transitively) the parameter's type (e.g. passing an `Armor` where a `Form` is expected, or an `Actor` where an `ObjectReference` is expected). Calls to functions declared in the same script are always checked; when linting a `.psc` file dropped in the app, calls to functions declared on other scripts under the project root (e.g. `SomeProperty.DoThing(...)`) are checked too, by resolving those scripts' signatures (including through `Extends`), and the `Extends` chain of an argument's own script is likewise resolved from the project root to allow compatible subtypes. Native engine types (e.g. `Actor`, `ObjectReference`, `Form`, `Spell`) whose own `.psc` isn't part of the project fall back to the common Skyrim/Fallout 4 native class hierarchy listed in `rules/native-types.yaml`, so a subtype relationship between them is still recognized. A call whose target or argument type can't be determined is skipped rather than guessed at.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::return_types::RULE,
        description: "Flags `Return` statements whose value's type doesn't match the enclosing function's declared return type (e.g. returning a `String` from a Function declared `Int`), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as returning an object whose script extends (directly or transitively) the declared return type (e.g. returning an `Armor` from a Function declared `Form`, or an `Actor` from a Function declared `ObjectReference`). When linting a `.psc` file dropped in the app, a returned value's own script's `Extends` chain is resolved from the project root to allow compatible subtypes there too, with the same native-type fallback used by the argument type check for engine types like `Actor`/`ObjectReference`/`Form`. A `Return` whose value's type can't be determined, or with no declared return type, is skipped rather than guessed at.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::function_override::RULE,
        description: "Flags, as an `[info]`, a function declared on this script that shares its name with a function declared on the script it `Extends` (directly or transitively) — the local declaration silently replaces the inherited one. This is often intentional (e.g. overriding an `Event OnInit()` handler), so it's informational rather than a warning. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked (state-based override is a separate mechanism from `Extends`).",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::argument_naming::RULE,
        description: "Flags, as a `[warning]`, a function declared on this script whose parameter name doesn't match (case-insensitively) the corresponding parameter of the same-named function declared on the script it `Extends` (directly or transitively) — since Papyrus resolves a named-argument call against the declared type of the reference it's called through, a renamed parameter on an override can silently misdirect (or fail to compile) a caller using the parent's names. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked, and only parameter positions present on both declarations are compared.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::state_function_signature::RULE,
        description: "Flags, as an `[error]`, a function or event declared inside a `State` block whose parameter count/types or return type doesn't match the same-named declaration in the script's \"empty state\" (the one declared directly on the script, outside any `State` block) — Papyrus requires these to match identically for the state version to be recognized as an override of the empty-state one at all, rather than becoming a distinct, effectively unreachable function. Only compared against an empty-state declaration already present on the script being linted; a state function may instead validly match one declared on a parent script (per the language spec), which this lint has no way to resolve, so that case is left unflagged.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::numeric_comparison::RULE,
        description: "Flags implicit comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) between an `Int` value and a `Float` value without an explicit cast making the comparison exact. Only comparisons whose operand types can be determined locally are checked.",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::indentation::RULE,
        description: "Flags, as a `[warning]`, lines whose indentation doesn't match the configured style/width (`indentation`/`indentation_width`) for their nesting depth. A script whose structure can't be identified (e.g. it doesn't lex cleanly) is left unchecked rather than guessed at.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::cyclomatic_complexity::RULE,
        description: "Flags functions/events whose cyclomatic complexity (1 plus each `If`/`ElseIf` branch, `While` loop, and short-circuiting `&&`/`||` operator) exceeds a configurable threshold, as a `[warning]` above `cyclomatic_complexity_warning` (default 10) or an `[error]` above `cyclomatic_complexity_error` (default 20).",
        kinds: &["maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unreachable_statement::RULE,
        description: "Flags statements that follow a `Return` within the same block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a `While` body), since they can never execute.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::static_condition::RULE,
        description: "Flags `If`/`ElseIf`/`While` conditions that fold to a constant `true` or `false` (e.g. `If true`, `If 1 == 2`, `If !false && 3 > 4`), regardless of any runtime state, as a `[warning]`. Only conditions built entirely from literals (combined with arithmetic, comparison, logical, and unary operators) are checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::division_by_zero::RULE,
        description: "Flags, as a `[warning]`, a `/` or `%` whose right-hand operand is a compile-time-constant zero (e.g. `x / 0`, `x % 0.0`, `x / (1 - 1)`), since that crashes the script at runtime. Only a divisor built entirely from literals (combined with arithmetic and unary operators) is checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::empty_body::RULE,
        description: "Flags, as a `[warning]`, since this is almost always a forgotten piece of logic rather than something intentional: a `While` loop whose body is empty, or whose body only nudges a variable by a constant amount (`i += 1`, `i -= 1`, or the equivalent `i = i + 1`/`i = i - 1`) with nothing else giving the loop a purpose (a step built from anything but a literal, such as a call or another variable, is left alone since it has a side effect of its own); and an empty `If`, `ElseIf`, or `Else` body. An `Else` clause is told apart from no `Else` clause at all (both parse to an empty body) by scanning for a literal `Else` immediately followed by `EndIf` in the source.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_local_variable::RULE,
        description: "Flags a local variable (declared with `Type name = ...` inside a function/event) whose value is never read: either it's never referenced again at all, or it's only ever reassigned (`name = ...`) without that new value ever being read back. Reading a variable via a compound assignment (`name += ...`, etc.) or through a member/index expression built from it (`name.Foo`, `name[0]`) counts as a use. Function parameters and script properties aren't locals and are never flagged by this lint.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::none_form_usage::RULE,
        description: "Flags a member/method access (`a.GetName()`, `a.Name`) on a local variable or script-level `Auto`/`AutoReadOnly` property that's still known to be `None` (e.g. `Armor a = None` followed directly by `a.GetName()`), since that crashes the script at runtime. An object-typed local without an initializer, or an object-typed `Auto`/`AutoReadOnly` property with no explicit default value (or an explicit `= None`), starts out `None` in every function, since it may not be set until something outside the script (the CK's Property Manager, another script, `OnInit`, …) does so — unless `assume_auto_properties_filled` is set, in which case a property doesn't start out possibly `None` on its own; it's still tracked the same as a local variable once script code assigns it `None` directly. From there, a variable/property is tracked as `None` from its declaration/assignment until it's reassigned something else, narrowing through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`||`) and through a `While` loop's condition (this language has no `break`/`continue`, so the loop can only exit once its condition is false). A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Anything less direct is left unflagged rather than guessed at.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::local_variable_shadowing::RULE,
        description: "Flags a local variable (declared with `Type name = ...` inside a function/event) whose name matches (case-insensitively) a `Property` declared on the same script, since referencing that name inside the function then reads the local rather than the property. When linting a `.psc` file dropped in the app, a local that instead shadows a property declared on a parent script (resolved through `Extends`) is flagged too.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::parameter_reassignment::RULE,
        description: "Flags, as a `[warning]`, a function/event parameter assigned a new value anywhere in its own body (`total = 1`, `total += 1`, ...), since reusing the parameter's name for a different value discards what the caller passed in and can confuse a reader expecting it to still reflect the original argument. A member/index assignment built from a parameter (`akRef.Foo = 1`), or a reassignment of an unrelated local variable, is never flagged.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::chain_whitespace::RULE,
        description: "Flags, as a `[warning]`, a space or tab immediately before or after a `.` member/method access (e.g. `SomeProperty . DoThing()`), since it interrupts the chain for no benefit. A `.` inside a `Float` literal (e.g. `1.5`) is never flagged. The fix closes the gap on whichever side(s) have it, without reaching across a newline (a chain continued onto another physical line is left alone).",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::exclamation_spacing::RULE,
        description: "Flags, as a `[warning]`, a `!` negation operator not followed by exactly one space (e.g. `!bReady` or `!  bReady`), since a bit of breathing room makes the negation easier to spot. Never flags `!=`, which the lexer tokenizes separately, nor the gap between two directly adjacent `!`s in a chained/double negation (e.g. `!!bReady`) — only the last `!` in such a run needs its own trailing space. The fix inserts a space where there is none and collapses a longer run of spaces/tabs down to one.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::identifier_casing::RULE,
        description: "Flags a declared function/event, property, state, parameter, or local/script variable whose name doesn't match the configured `identifier_casing` style: `camelCase`, `PascalCase`, `snake_case`, or `CONSTANT_CASE`. `ScriptName` itself is never checked by this lint (see \"Type name casing\" below). A parameter has no location of its own, so it's reported on its enclosing function's line. The automatic fix renames each flagged declaration and its references only when the conversion preserves every underscore in its original position; fixes that would add, remove, or move underscores are left for the user because they constitute a substantive rename.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::type_casing::RULE,
        description: "Flags, as a `[warning]`, a script's declared type name (the identifier following `ScriptName`) if it doesn't follow the configured `type_casing` convention (`PascalCase`, `camelCase`, `lowercase`, or `UPPERCASE`). Only the script's own declared name is checked and fixed, never its `Extends` target, since that type is declared (and presumably already checked) in another script. Up to two leading acronym-prefix segments (one or more uppercase letters followed by one or more underscores, e.g. CreationKit's own `IDR__TIF__050000F5` dialogue fragment names or a modder's own `USSEP_` acronym prefix) are ignored, since that part of the name can't be renamed; only the rest of the name is checked and fixed. The fix only changes letter casing, preserving the name's characters so it remains compatible with its `.psc` filename; a violation that would require a substantive rename (such as removing an underscore for `PascalCase`) is left for the user to rename together with the file.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::named_arguments::RULE,
        description: "Flags, as a `[warning]`, a positional call argument that the configured `named_arguments` setting prefers to see passed by Papyrus's named-argument syntax instead (`func(argB = 1)`): `always` flags every positional argument, `instead_of_defaults` flags only an argument filling a parameter that has a default value, and `never` (the default) flags nothing. Parameter names and default values are only known for functions declared in the script being linted (including via `self.Func(...)`), so a call to a function declared on another script is never flagged. An argument already passed by name is always accepted regardless of setting.",
        kinds: &["style", "maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::operator_spacing::RULE,
        description: "Requires, as a `[warning]`, exactly one space on either side of `&&`, `||`, `==`, `!=`, `>`, `<`, `>=`, and `<=`. A side whose whitespace reaches a newline (the operator opens or closes a statement continued across physical lines) is left unchecked on that side. The fix normalizes each flagged side to a single space, without reaching across a newline.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::property_sorting::RULE,
        description: "Flags, as a `[warning]`, a `Property` declaration that isn't sorted by type and then alphabetically by name, or that isn't declared immediately after the `ScriptName` line, before any variable, function, or state declaration (an `Import` isn't tracked closely enough to count against this). Disabled by default, since reordering a script's declared properties is a more invasive change than the rest of these lints; a project opts in via `rules.property_sorting`. The fix relocates each property's own declaration lines (its full `Property`/`EndProperty` block, for a non-auto property) as a group right after `ScriptName`, in sorted order; a documentation comment placed directly above a property is left behind rather than moved with it.",
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::explicit_return::RULE,
        description: "Flags, as an `[error]`, a typed function/event with a code path that falls off the end of its body without a `Return`, since Papyrus then silently returns that type's default value (`0`, `\"\"`, `False`, or `None`) instead of one the author chose. A `Return` with no value still counts as long as it's reached (`return_types` covers a value's actual type); an `If` only counts when every branch, including an `Else`, returns, and a `While` loop is never assumed to guarantee one since it may run zero times. A native function has no body to inspect and is never flagged.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::unchecked_form_parameter::RULE,
        description: "Flags, as a `[warning]`, a member/method access (`akForm.GetName()`) on a `Form`-typed function parameter that hasn't yet been confirmed non-`None` in that path, since a caller can always pass in `None` and dereferencing it crashes the script at runtime. Tracks a parameter as unconfirmed from the start of its function until it's narrowed through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`||`) or a `While` loop's condition, the same way \"None used as an existing Form\" above narrows its own state, or until it's reassigned to anything else. A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Passing the parameter on as an argument to another call isn't flagged, only a direct member/method access is. Disabled by default, since many scripts intentionally accept a possibly-`None` Form and defer the check to a caller or a later branch; a project opts in via `rules.unchecked_form_parameter`.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::unchecked_cast::RULE,
        description: "Flags, as a `[warning]`, a member/method access on the result of an `as` cast (e.g. `(akRef as Actor).GetActorValue(\"Health\")`) before that result has been checked against `None`, since a cast that doesn't match the underlying Form's actual type evaluates to `None` at runtime rather than raising an error, so dereferencing it immediately crashes the script. Tracks a local variable as an unchecked cast result from its declaration/assignment from an `as` expression until it's reassigned something else, clearing it the moment a direct `None` check on it (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`||`) is evaluated, regardless of which branch is ultimately taken — this lint only cares whether the possibility of `None` was ever considered, not which branch handles it. A cast used directly inline (`(value as Type).Member`) is always flagged, since there's no way to check it in between. A cast CreationKit itself generated (the boilerplate line a quest/dialogue fragment gets between its `Function` signature and `;BEGIN CODE`, e.g. `Actor akSpeaker = akSpeakerRef as Actor`) is never tracked as unchecked in the first place, since CreationKit guarantees that cast succeeds and the user can't add a `None` check there without CreationKit rejecting the edit.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::useless_downcast::RULE,
        description: "Flags, as an `[info]`, an explicit `as` cast that can't actually narrow anything: either its target type exactly matches the value's already-known type, or the value's type already extends the target (directly or transitively) — e.g. `Actor dude` followed by `Foo(dude as ObjectReference)`, since `Actor` already extends `ObjectReference` and Papyrus would accept `dude` there without the cast. Only a cast whose value's type can be determined locally (locals, parameters, properties, `Self`/`Parent`, literals, and other resolvable expressions) is checked; a member access or function call result is left unflagged rather than guessed at. Primitive types (`Int`, `Float`, `Bool`, `String`) are only flagged for an exact-type cast, never treated as extending one another, so a meaningful conversion like an explicit `Int`-to-`Float` widening cast is never flagged. When linting a `.psc` file dropped in the app, a cast target that's an ancestor of the value's script (rather than an exact match) is resolved the same way the argument/return type checks resolve their own, including through the native engine type fallback for types like `Actor`/`ObjectReference`/`Form`.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::unresolved_script::RULE,
        description: "Flags, as a `[warning]`, an unresolved parent in `Extends`, an unresolved type annotation, or a call through Papyrus's static/global call syntax (e.g. `MyMissingScript.DoThing()`) whose target script can't be found. Primitive types and native engine types are recognized without project-side source. Only a call whose object is a bare identifier not already known as a local variable, parameter, or property is considered a script reference at all — one resolved through a variable or property is left to the \"Argument type check\"/\"Return type check\" lints instead. Only checked when linting with project context, by resolving names against `.psc` files under the project root the same way the argument/return type checks do, with native types and singleton scripts supplied by the built-in rule data.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::non_global_function_call::RULE,
        description: "Flags, as an `[error]`, a call through Papyrus's static/global call syntax (e.g. `MyScript.DoThing()`) whose target function resolves but isn't declared `Global` on that script, since Papyrus only allows that syntax to reach a script's `Global` functions — calling an ordinary instance function that way fails to compile. Uses the same \"bare identifier not already known as a local variable, parameter, or property\" rule as the \"Unresolved script reference\" lint above to tell a script reference apart from an instance call; a call whose script or function can't be resolved at all is left unflagged (see that lint instead). Only checked when linting with project context, by resolving the target script's functions the same way the argument/return type checks do.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::static_function_call_via_instance::RULE,
        description: "Flags, as a `[warning]`, the mirror case of \"Non-static function call\" above: a call reaching a `Global` function through an actual object reference (a local variable, parameter, property, cast, or array element, e.g. `akOtherActor.MyGlobalHelper()`) instead of Papyrus's static/global call syntax (`MyScript.MyGlobalHelper()`). Papyrus allows this — a `Global` function ignores whatever reference it's called through — but it can read as a mistake, since a reader (or the author, if the call was copy-pasted from an instance method) may expect the object to matter. `Self`/`Parent` are never flagged, since calling a script's own `Global` function through `Self` for symmetry with its other `Self.Whatever()` calls is a reasonable, common style rather than a likely mistake. Only checked when linting with project context, by resolving the target script's functions the same way the argument/return type checks do.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::short_wait_interval::RULE,
        description: "Flags, as a `[warning]`, a call to `Utility.Wait`, `RegisterForUpdate`, `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or `RegisterForSingleUpdateGameTime` whose interval argument folds to a compile-time-constant number below the configurable `min_wait_interval` (default `0.1`), since an interval that short runs far more often than is typically useful and can add up to meaningful performance overhead. `Utility.Wait` is only matched when qualified by that literal script name, the same way the \"Forbidden/discouraged function usage\" lint treats native singletons; the `RegisterFor*` family matches unqualified or through any receiver. Only an argument built entirely from literals (combined with arithmetic and unary operators) is checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at.",
        kinds: &["performance"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::goto_state::RULE,
        description: "Flags, as a `[warning]`, a `GoToState(\"Name\")` call (bare or `self.GoToState(...)`) whose target isn't declared as a `State` on this script, since a typo'd or renamed state name still compiles — the engine just silently falls back through its state resolution algorithm instead of raising an error, so the call quietly never takes effect. `GoToState(\"\")`, which switches back to the empty state, is always valid. Only a literal string argument is checked; one built from anything else is left unflagged rather than guessed at. A target undeclared on this script is only flagged when this script has no `Extends` target at all, since it may otherwise be declared on a script further up that (unresolved) `Extends` chain — a legitimate way to forward-declare a state for a not-yet-written child script to implement. When linting a `.psc` file dropped in the app, that chain is resolved from the project root too, the same way the argument/return type checks resolve their own, so a target declared on an ancestor script is recognized rather than flagged.",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::state_count::TOO_MANY_STATES_RULE,
        description: "Flags, as an `[error]`, a script whose named `State` blocks, combined with every `State` declared anywhere in its `Extends` ancestry (a same-named state declared more than once along the way counts once), exceed 127 — the [CreationKit wiki's State Reference](https://ck.uesp.net/wiki/State_Reference) documents a hard engine limit of 128 states including the empty state, past which the game and CK refuse to load the script. Only the script's own declared states are counted when linting in isolation; when linting a `.psc` file dropped in the app, its `Extends` ancestry is resolved from the project root too, the same way the argument/return type checks resolve their own.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::state_count::MULTIPLE_AUTO_STATES_RULE,
        description: "Flags more than one `Auto` state declared in a single script as an `[error]`, since a script may only declare one. It also flags, as a `[warning]`, multiple `Auto` states found only after combining a script with every `State` declared anywhere in its `Extends` ancestry (as above). The engine tolerates a parent and child each declaring an `Auto` state (the child's takes precedence at startup), but relying on that precedence is fragile: removing the child's `Auto` state silently switches its startup state back to the parent's. Only the script's own declared states are considered when linting in isolation; when linting a `.psc` file dropped in the app, its `Extends` ancestry is resolved from the project root too.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: "conflicting-script-versions",
        description: "Flags, as a `[warning]`, a `.psc` file when another script search directory contains a case-insensitively same-named file with different contents (determined by MD5), since which version Papyrus resolves can depend on search-directory order. Byte-identical copies are ignored. Only available when linting a file with project context in the desktop app or CLI.",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_disable::RULE,
        description: "Flags, as a `[warning]`, each rule id in an `@disable`/`@disable-file` comment that is unknown or does not suppress a diagnostic from that rule (on its line for `@disable`, anywhere in the file for `@disable-file`). A bare `@disable` is flagged when its line has no diagnostics to suppress; a bare `@disable-file` is flagged when the whole file has none. Disabled by default; opt in with `rules.unused_disable`.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::magic_numbers::RULE,
        description: "Flags, as a `[warning]`, a numeric literal used directly in an expression rather than through a named constant, property, or local variable. `-1`, `0`, and `1` are never flagged, since they're near-universally used directly without losing any clarity. A literal that's the entire value given to a declaration or assignment (`Int kMaxTargets = 5`, later reassigned as `kMaxTargets = 6`) is left alone too, since naming it there already gives it the meaning this lint is after; a literal nested inside a more complex initializer (`Int kMaxTargets = 5 + 1`) is still checked. Disabled by default; opt in with `rules.magic_numbers`. The configurable `magic_numbers` setting controls how a `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/`RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call's interval argument is treated: `loose` (the default) leaves it unflagged, since a hardcoded interval there is common and usually self-explanatory; `strict` checks it like any other argument.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::variable_used_before_assignment::RULE,
        description: "Flags, as a `[warning]`, a read of a local variable declared without an initial value (`Int i` rather than `Int i = 0`) before anything in the enclosing function ever assigns it one, since the read then actually observes Papyrus's implicit per-type default (`0`, `0.0`, `False`, `\"\"`, or `None`) rather than a value the author chose. A variable is tracked as unassigned from its declaration until a plain `name = value` assignment reaches it; a compound assignment (`name += 1`, ...) is flagged too, since it reads the still-default value before writing the new one, and then counts as assigned from that point on. `If`/`ElseIf`/`Else` branches are each checked from the same incoming state, and a branch that unconditionally `Return`s doesn't carry its state past the `If`; a variable assigned by any surviving branch counts as assigned afterward too, so a variable assigned in only one branch (or with no `Else` at all) and later tested against its default to see whether that branch ran isn't flagged — only a variable left unassigned by every surviving branch still is. Since a `While` loop may run zero times, an assignment made only inside its body is never assumed to have happened by the time execution reaches the code after the loop. Function parameters and script properties always have a value by the time a function runs and are never flagged by this lint. An `==`/`!=` comparison against that same variable's own declared-type default (`None`, `0`, `0.0`, `False`, or `\"\"`) is treated as a deliberate \"has this been set yet?\" gate rather than a genuine read, so it's never flagged either — a mismatched comparison like `Int i` against `None`, or `Bool b` against `0`, isn't that type's default and is still flagged (other reads of the same variable still are too). That gate exemption also carries across a short-circuiting `&&`/`||` joining it to a further operand (e.g. `x == None || x.Foo()`, or `x != None && x.Foo()`), since that operand only ever evaluates once the gate has established the variable is no longer at its default.",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::native_function_usage::RULE,
        description: "Flags, as a `[warning]`, a `Native` function/event declared on a script whose name isn't one of the base game's own native functions, listed in `rules/native-methods.yaml` — a strong signal it's instead supplied by SKSE/F4SE or some other native extension the project depends on. Disabled by default, since plenty of mods intentionally depend on such an extension and don't need to be warned about it; opt in with `rules.native_function_usage`.",
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::repeated_getvalue::RULE,
        description: "Flags, as an `[info]`, a `GetValue()` call repeated on the same receiver across the conditions of a single `If`/`ElseIf` chain (e.g. `If gv.GetValue() == 1.0` / `ElseIf gv.GetValue() == 2.0`), since none of the chain's earlier branch bodies run before a later condition is evaluated, so the value can't have changed between those reads — it can be read into a local variable once ahead of the chain instead. Like \"Slow function usage\", a call's receiver can't generally be resolved back to a `GlobalVariable`-typed script, so this matches by the `GetValue` method name alone (case-insensitively, with no arguments); it's the only native method with that name (see `rules/native-methods.yaml`), so this doesn't misfire on unrelated types. Disabled by default, since a chain that reads the same global more than once is often written that way deliberately for readability and the performance cost is usually negligible outside a hot code path; a project opts in via `rules.repeated_getvalue`.",
        kinds: &["performance"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::global_variable_setvalue::RULE,
        description: "Flags, as a `[warning]`, a `SetValue`/`SetValueInt` call on a `GlobalVariable`-like receiver that writes a value an enclosing `If`/`ElseIf`/`Else` chain never proves is different from the value already there — either a branch writing back the exact literal its own `GetValue()`/`GetValueInt() == literal` condition just confirmed is already current, or the trailing `Else` of a chain that reads the same receiver elsewhere writing a literal with no condition of its own ruling out that value already being current, e.g. an `Else` unconditionally calling `gv.SetValue(0.0)` after an `If gv.GetValue() == 1.0` branch, where it should usually become an explicit `ElseIf gv.GetValue() != 0.0` instead. Only a `SetValue`/`SetValueInt` call standing alone as its own statement, guarded by a plain equality check against a literal, is considered; anything less direct is left unflagged rather than guessed at. Disabled by default, since the `Else` case is a heuristic rather than a proven no-op; opt in with `rules.global_variable_setvalue`.",
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::invariant_loop_condition::RULE,
        description: "Flags, as a `[warning]`, a `While` loop whose condition depends on a local variable or parameter that's never assigned (plainly or via a compound `+=`/`-=`/etc.) anywhere in the loop's own body, since a local variable/parameter can only ever change through a direct assignment inside the function that owns it — nothing else can reach in and modify it — so the condition can then never change once the loop starts: it either never runs at all, or never stops. Only a condition built entirely from identifiers, literals, and arithmetic/comparison/logical/unary operators is checked, the same restriction \"Static condition\" and \"Division by zero\" place on their own expressions; one reaching a call, a member/index access, `Self`/`Parent`, a cast, or a `new` array is left unflagged rather than guessed at, since its value may depend on state this lint can't see change (e.g. `a.IsDead()`, which can start returning something different purely because of what happens inside that call). An identifier that isn't a known local variable or parameter of the enclosing function — most notably a script `Property`, which another function or the engine is free to change at any time — disqualifies the whole condition rather than being assumed safe. This deliberately doesn't catch a loop that reassigns its exit condition to the very same value on every iteration (e.g. re-fetching a reference that just happens to keep coming back dead), since only running the script could prove that.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::script_name_collision::RULE,
        description: "Flags, as an `[error]`, a script-level `Property` or variable whose name matches (case-insensitively) the name of the script it's declared in, since Papyrus rejects such a script at compile time. A local variable declared inside a function/event (see \"Local variable shadowing\" above) isn't checked by this lint.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::array_bounds::RULE,
        description: "Flags, as a `[warning]`, a literal index into a local array variable that falls outside the compile-time-constant size it was declared with (e.g. `Float[] a = new Float[3]` followed by `a[5] = 0.1`), since Papyrus doesn't raise a catchable error for an out-of-range array access — it just logs the mistake and silently no-ops the write or returns the type's default for a read. A variable starts tracking a size the moment it's assigned `new <Type>[<N>]` for a literal (or literal-arithmetic) `N`, and stops being tracked the moment it's assigned anything else; a size is only kept past an `If` when every surviving branch agrees on it, and a size learned only inside a `While` loop's body is never assumed to still hold afterward, since the loop may run zero times. Only a plain identifier's own index is checked; a member/property array, or an index built from anything other than a literal (optionally combined with arithmetic and unary operators), is left unflagged rather than guessed at. Independently of a tracked size, a `new <Type>[<N>]` whose own literal `N` falls outside `0` to `128` is always flagged, since Papyrus hard-caps an array created with `New` (or grown with `Add()`) at 128 elements; that cap doesn't apply to an array returned by a native function or to an editor-populated array `Property`, so indexing one of those is never flagged against it — only against a locally tracked `new` size, per above.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::readonly_property_write::RULE,
        description: "Flags, as an `[error]`, an assignment (`=`, `+=`, `-=`, ...) targeting a script-level property declared `AutoReadOnly` (e.g. `Float Property a = 0.1 AutoReadOnly`), since Papyrus rejects that assignment at compile time — an `AutoReadOnly` property can only ever hold its declared initial value. Matched by the property's bare name or as `Self.PropertyName`; a bare name shadowed by a same-named local variable or parameter in the enclosing function refers to that local/parameter instead and is never flagged, while a `Self.`-qualified write is always flagged regardless of shadowing.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::default_property_value::RULE,
        description: "Flags, as a `[warning]`, a `Bool`/`Int`/`Float`/`String` `Auto`/`AutoReadOnly` property declared with no explicit default value (e.g. `Int Property Count Auto` rather than `Int Property Count = 0 Auto`), since it then silently falls back to Papyrus's own implicit per-type default (`False`, `0`, `0.0`, or `\"\"`) instead of a value the author actually chose. Object-typed properties, array-typed properties, and full (non-`Auto`/`AutoReadOnly`) properties are never flagged. Disabled by default, since many existing scripts already rely on Papyrus's implicit defaults for some or all of their properties; opt in with `rules.default_property_value`.",
        kinds: &["style", "maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::unguarded_self_recursion::RULE,
        description: "Flags, as a `[warning]`, a function/event that calls itself with nothing that could ever skip the recursive call on some invocation, since without one it recurses unconditionally, every single time, until the call stack is exhausted. Deliberately conservative: it never follows a call chain through another function, and a self-call nested directly inside an `If`'s or `While`'s own body is always left alone, since being inside a branch or loop body already makes that call conditional. At the function's top level, any `While` still disqualifies the whole function from this lint, but a top-level `If` only counts as a guard — and so only disqualifies the function — when it actually contains a `Return` somewhere within it (searched recursively through any further nested `If`/`While`, since a guard's early exit can be buried behind further branching); an `If` with no `Return` anywhere inside it does nothing to actually stop the recursive call that follows it, so it's still flagged. This still doesn't evaluate whether a guard's condition is actually correct, only that a `Return` exists for it to possibly take. A self-call reached only through the right-hand side of a short-circuiting `&&`/`||` doesn't count either, since that side isn't guaranteed to evaluate. A `GoToState(...)` to a different state that precedes the self-call in the same body also counts as a guard, but only when that other state actually declares its own handler for the same function/event — otherwise, dispatch would fall back to the empty state's declaration, which may still be this exact function, so the call is still flagged. This covers Bethesda's standard save-compatibility idiom, where a state switch right before the \"recursive\" call re-points dispatch at another (typically empty) handler instead of actually re-entering this body.",
        kinds: &["correctness"],
        importance: Importance::High,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KNOWN_RULE_IDS;
    use std::collections::HashSet;

    #[test]
    fn every_known_rule_id_has_tags() {
        for rule in KNOWN_RULE_IDS {
            assert!(
                tags_for(rule).is_some(),
                "{rule:?} is in KNOWN_RULE_IDS but has no RULE_TAGS entry"
            );
        }
    }

    #[test]
    fn every_tagged_rule_is_a_known_rule_id() {
        for tags in RULE_TAGS {
            assert!(
                KNOWN_RULE_IDS.contains(&tags.rule),
                "{:?} has a RULE_TAGS entry but isn't in KNOWN_RULE_IDS",
                tags.rule
            );
        }
    }

    #[test]
    fn rule_tags_has_no_duplicate_rule_ids() {
        let mut seen = HashSet::new();
        for tags in RULE_TAGS {
            assert!(
                seen.insert(tags.rule),
                "{:?} appears more than once in RULE_TAGS",
                tags.rule
            );
        }
    }

    #[test]
    fn every_rule_has_at_least_one_kind() {
        for tags in RULE_TAGS {
            assert!(
                !tags.kinds.is_empty(),
                "{:?} has no kind keywords",
                tags.rule
            );
        }
    }

    #[test]
    fn every_rule_has_a_description() {
        for tags in RULE_TAGS {
            assert!(
                !tags.description.is_empty(),
                "{:?} has no description",
                tags.rule
            );
        }
    }

    #[test]
    fn auto_fixable_matches_fixable_rule_ids() {
        for tags in RULE_TAGS {
            assert_eq!(
                tags.auto_fixable(),
                FIXABLE_RULE_IDS.contains(&tags.rule),
                "{:?}.auto_fixable() disagrees with FIXABLE_RULE_IDS",
                tags.rule
            );
        }
    }

    #[test]
    fn tags_for_matches_case_insensitively() {
        assert_eq!(
            tags_for("TRAILING-WHITESPACE").map(|t| t.rule),
            Some(crate::trailing_whitespace::RULE)
        );
    }

    #[test]
    fn tags_for_returns_none_for_an_unknown_rule() {
        assert!(tags_for("made-up-rule").is_none());
    }
}
