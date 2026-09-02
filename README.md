# Papyrus Lint [![Quality gate status](https://sonarcloud.io/api/project_badges/measure?project=Idrinth_papyrus-lint&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=Idrinth_papyrus-lint) [![Discord Server](https://img.shields.io/badge/discord-server-5865F2?logo=discord)](link=https://discord.gg/idrinth) [![NexusMods](https://img.shields.io/badge/nexusmods-page-yellow)](https://www.nexusmods.com/skyrimspecialedition/mods/189862) [![GitHub](https://img.shields.io/badge/github-repo-white?logo=github)](https://github.com/idrinth/papyrus-lint)

![Papyrus Lint logo](resources/logo-small.jpg)

**Papyrus Lint catches bugs that CreationKit's compiler lets through.**
`PapyrusCompiler.exe` only checks that a script is syntactically valid — it
will happily compile a script that dereferences a `None` object at
runtime, passes a `String` where an `Int` is expected, returns the wrong
type from a function, compares an `Int` to a `Float` inexactly, or
contains a branch or statement that can never execute. Those bugs don't
show up until later, as a CTD, a quest stage that never advances, or a
value that's silently wrong — often far from the line that actually
caused them, and long after the mod author has lost the context to spot
it. Papyrus Lint scans your `.psc` source for exactly these patterns (see
the full list below) before you ship, on top of the formatting/style
issues a linter usually catches, so a mod author finds them at write time
instead of from a bug report.

## Simple Example

```papyrus
Function DoSomething(Actor a, Actor b, Form SomeItem)
    a.RemoveItem(SomeItem, 1, b);
EndFunction
```

This compiles, but the likely desired version is:

```papyrus
Function DoSomething(Actor a, Actor b, Form SomeItem)
    a.RemoveItem(SomeItem, 1, false, b);
EndFunction
```

The strict boolean check identifies this issue because the first call passes
the `Actor` value `b` to a `Bool` parameter.

![Papyrus Lint Results](resources/papyrus-lint-results.png)

## What this is NOT

- A compiler
- An editor
- A guarantee a script is fit for purpose
- A replacement for proper testing

## What is a linter?

A linter is a tool that scans source code for patterns that are likely to be
mistakes, bad practice, or inconsistent style, without actually running the
code. It works from heuristics — recognizable patterns known to often cause
problems — rather than proving that a given line is definitely wrong.
Because of that, a linter can produce **false positives**: diagnostics on
code that is actually fine, especially for patterns the checks intentionally
can't fully resolve (see e.g. "Strict boolean check" or "None used as an
existing Form" below, which skip anything they can't determine with
confidence rather than guess). It's normal to disagree with an individual
diagnostic and dismiss it.

Treat every diagnostic here as **advice, not a guaranteed defect report**: a
suggestion worth a second look, not proof the code is broken. Use your own
judgment for whether a flagged line needs changing, and use the
[`; @disable`](#disabling-a-lint-on-a-specific-line) comment below to
silence a specific rule on a specific line when you've decided it doesn't
apply.

## Implemented Lints

### Formatting

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Trailing whitespace** | Flags, as a `[warning]`, lines that end with trailing spaces or tabs. | ✓ |
| **Space after comma** | Requires, as a `[warning]`, whitespace after commas in argument lists. | ✓ |
| **Semicolon at end of line** | Requires, as a `[warning]`, a trailing semicolon on each non-empty line or forbids terminal semicolons, according to the selected setting. | ✓ |
| **Formatting checks** | Flags, as a `[warning]`, lines whose indentation doesn't match the configured style/width (`indentation`/`indentation_width`) for their nesting depth. A script whose structure can't be identified (e.g. it doesn't lex cleanly) is left unchecked rather than guessed at. | ✓ |
| **Whitespace interrupting property/method chaining** | Flags, as an `[error]`, a space or tab immediately before or after a `.` member/method access (e.g. `SomeProperty . DoThing()`), since it interrupts the chain for no benefit. A `.` inside a `Float` literal (e.g. `1.5`) is never flagged. The fix closes the gap on whichever side(s) have it, without reaching across a newline (a chain continued onto another physical line is left alone). | ✓ |
| **Exclamation mark spacing** | Flags, as a `[warning]`, a `!` negation operator not followed by exactly one space (e.g. `!bReady` or `!  bReady`), since a bit of breathing room makes the negation easier to spot. Never flags `!=`, which the lexer tokenizes separately. The fix inserts a space where there is none and collapses a longer run of spaces/tabs down to one. | ✓ |
| **Type name casing** | Flags, as a `[warning]`, a script's declared type name (the identifier following `ScriptName`) if it doesn't follow the configured `type_casing` convention (`PascalCase`, `camelCase`, `lowercase`, or `UPPERCASE`). Only the script's own declared name is checked and fixed, never its `Extends` target, since that type is declared (and presumably already checked) in another script. The fix only changes letter casing, preserving the name's characters so it remains compatible with its `.psc` filename; a violation that would require a substantive rename (such as removing an underscore for `PascalCase`) is left for the user to rename together with the file. | ✓ |
| **Identifier casing** | Flags a declared function/event, property, state, parameter, or local/script variable whose name doesn't match the configured `identifier_casing` style: `camelCase`, `PascalCase`, `snake_case`, or `CONSTANT_CASE`. `ScriptName` itself is never checked by this lint (see "Type name casing" below). A parameter has no location of its own, so it's reported on its enclosing function's line. The automatic fix renames each flagged declaration and its references only when the conversion preserves every underscore in its original position; fixes that would add, remove, or move underscores are left for the user because they constitute a substantive rename. | ✓ |
| **Spacing around logical/comparison operators** | Requires, as a `[warning]`, exactly one space on either side of `&&`, `\|\|`, `==`, `!=`, `>`, `<`, `>=`, and `<=`. A side whose whitespace reaches a newline (the operator opens or closes a statement continued across physical lines) is left unchecked on that side. The fix normalizes each flagged side to a single space, without reaching across a newline. | ✓ |
| **Property sorting** | Flags, as a `[warning]`, a `Property` declaration that isn't sorted by type and then alphabetically by name, or that isn't declared immediately after the `ScriptName` line, before any variable, function, or state declaration (an `Import` isn't tracked closely enough to count against this). Disabled by default, since reordering a script's declared properties is a more invasive change than the rest of these lints; a project opts in via `rules.property_sorting`. The fix relocates each property's own declaration lines (its full `Property`/`EndProperty` block, for a non-auto property) as a group right after `ScriptName`, in sorted order; a documentation comment placed directly above a property is left behind rather than moved with it. | ✓ |

### Performance

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Forbidden/discouraged function usage** | Flags calls to functions listed in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls), with a configurable severity and an explanatory message per entry. | |
| **Slow function usage** | Flags calls to functions listed in `rules/slow-functions.yaml` that have a faster equivalent available, and suggests the quicker alternative. | |
| **Short wait/update interval** | Flags, as a `[warning]`, a call to `Utility.Wait`, `RegisterForUpdate`, `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or `RegisterForSingleUpdateGameTime` whose interval argument folds to a compile-time-constant number below the configurable `min_wait_interval` (default `0.1`), since an interval that short runs far more often than is typically useful and can add up to meaningful performance overhead. `Utility.Wait` is only matched when qualified by that literal script name, the same way the "Forbidden/discouraged function usage" lint treats native singletons; the `RegisterFor*` family matches unqualified or through any receiver. Only an argument built entirely from literals (combined with arithmetic and unary operators) is checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |

### Reliability

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Getter usage without saving result** | Flags calls to functions whose names begin with `Get` (case-insensitively) whose result is discarded, whether the call stands alone (`GetValue()`) or only feeds a comparison, arithmetic, or logical operator whose own result is then discarded too (e.g. `GetDistance(target) > 0` on its own line, with no assignment, `Return`, or condition around it). | |
| **Strict boolean check** | Flags `If`/`ElseIf`/`While` conditions that aren't already a `Bool` value or expression, instead of relying on Papyrus's implicit conversion to boolean. Only conditions whose type can be determined locally (locals, parameters, properties, literals, casts, and comparison/logical expressions) are checked; a condition that depends on a function call or a member access is left unflagged rather than risk a false positive. | |
| **Argument type check** | Flags call-site arguments whose type doesn't match the callee's declared parameter type (e.g. passing a `String` where an `Int` is expected), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as passing an object whose script extends (directly or transitively) the parameter's type (e.g. passing an `Armor` where a `Form` is expected, or an `Actor` where an `ObjectReference` is expected). Calls to functions declared in the same script are always checked; when linting a `.psc` file dropped in the app, calls to functions declared on other scripts under the project root (e.g. `SomeProperty.DoThing(...)`) are checked too, by resolving those scripts' signatures (including through `Extends`), and the `Extends` chain of an argument's own script is likewise resolved from the project root to allow compatible subtypes. Native engine types (e.g. `Actor`, `ObjectReference`, `Form`, `Spell`) whose own `.psc` isn't part of the project fall back to the common Skyrim/Fallout 4 native class hierarchy listed in `rules/native-types.yaml`, so a subtype relationship between them is still recognized. A call whose target or argument type can't be determined is skipped rather than guessed at. | |
| **Return type check** | Flags `Return` statements whose value's type doesn't match the enclosing function's declared return type (e.g. returning a `String` from a Function declared `Int`), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as returning an object whose script extends (directly or transitively) the declared return type (e.g. returning an `Armor` from a Function declared `Form`, or an `Actor` from a Function declared `ObjectReference`). When linting a `.psc` file dropped in the app, a returned value's own script's `Extends` chain is resolved from the project root to allow compatible subtypes there too, with the same native-type fallback used by the argument type check for engine types like `Actor`/`ObjectReference`/`Form`. A `Return` whose value's type can't be determined, or with no declared return type, is skipped rather than guessed at. | |
| **Inherited function override** | Flags, as an `[info]`, a function declared on this script that shares its name with a function declared on the script it `Extends` (directly or transitively) — the local declaration silently replaces the inherited one. This is often intentional (e.g. overriding an `Event OnInit()` handler), so it's informational rather than a warning. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked (state-based override is a separate mechanism from `Extends`). | |
| **Argument naming consistency** | Flags, as a `[warning]`, a function declared on this script whose parameter name doesn't match (case-insensitively) the corresponding parameter of the same-named function declared on the script it `Extends` (directly or transitively) — since Papyrus resolves a named-argument call against the declared type of the reference it's called through, a renamed parameter on an override can silently misdirect (or fail to compile) a caller using the parent's names. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked, and only parameter positions present on both declarations are compared. | |
| **State function signature mismatch** | Flags, as an `[error]`, a function or event declared inside a `State` block whose parameter count/types or return type doesn't match the same-named declaration in the script's "empty state" (the one declared directly on the script, outside any `State` block) — Papyrus requires these to match identically for the state version to be recognized as an override of the empty-state one at all, rather than becoming a distinct, effectively unreachable function. Only compared against an empty-state declaration already present on the script being linted; a state function may instead validly match one declared on a parent script (per the language spec), which this lint has no way to resolve, so that case is left unflagged. | |
| **Strict numeric type check** | Flags implicit comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) between an `Int` value and a `Float` value without an explicit cast making the comparison exact. Only comparisons whose operand types can be determined locally are checked. | |
| **Explicit return on every path** | Flags, as an `[error]`, a typed function/event with a code path that falls off the end of its body without a `Return`, since Papyrus then silently returns that type's default value (`0`, `""`, `False`, or `None`) instead of one the author chose. A `Return` with no value still counts as long as it's reached (`return_types` covers a value's actual type); an `If` only counts when every branch, including an `Else`, returns, and a `While` loop is never assumed to guarantee one since it may run zero times. A native function has no body to inspect and is never flagged. | |
| **Unresolved script reference** | Flags, as a `[warning]`, an unresolved parent in `Extends`, an unresolved type annotation, or a call through Papyrus's static/global call syntax (e.g. `MyMissingScript.DoThing()`) whose target script can't be found. Primitive types and native engine types are recognized without project-side source. Only a call whose object is a bare identifier not already known as a local variable, parameter, or property is considered a script reference at all — one resolved through a variable or property is left to the "Argument type check"/"Return type check" lints instead. Only checked when linting with project context, by resolving names against `.psc` files under the project root the same way the argument/return type checks do, with native types and singleton scripts supplied by the built-in rule data. | |
| **GoToState state reference** | Flags, as a `[warning]`, a `GoToState("Name")` call (bare or `self.GoToState(...)`) whose target isn't declared as a `State` on this script, since a typo'd or renamed state name still compiles — the engine just silently falls back through its state resolution algorithm instead of raising an error, so the call quietly never takes effect. `GoToState("")`, which switches back to the empty state, is always valid. Only a literal string argument is checked; one built from anything else is left unflagged rather than guessed at. A target undeclared on this script is only flagged when this script has no `Extends` target at all, since it may otherwise be declared on a script further up that (unresolved) `Extends` chain — a legitimate way to forward-declare a state for a not-yet-written child script to implement. When linting a `.psc` file dropped in the app, that chain is resolved from the project root too, the same way the argument/return type checks resolve their own, so a target declared on an ancestor script is recognized rather than flagged. | |
| **Total named state count** | Flags, as an `[error]`, a script whose named `State` blocks, combined with every `State` declared anywhere in its `Extends` ancestry (a same-named state declared more than once along the way counts once), exceed 127 — the [CreationKit wiki's State Reference](https://ck.uesp.net/wiki/State_Reference) documents a hard engine limit of 128 states including the empty state, past which the game and CK refuse to load the script. Only the script's own declared states are counted when linting in isolation; when linting a `.psc` file dropped in the app, its `Extends` ancestry is resolved from the project root too, the same way the argument/return type checks resolve their own. | |
| **Multiple Auto states** | Flags, as an `[error]`, a script whose `State` blocks, combined with every `State` declared anywhere in its `Extends` ancestry (as above), include more than one marked `Auto`. The engine itself tolerates a parent and a child each declaring their own `Auto` state (the child's simply takes precedence at startup), but relying on that precedence is fragile — which one actually applies silently depends on which script the instance is, and removing the child's `Auto` state later silently switches its startup state back to the parent's — so this lint flags the combination outright. Only the script's own declared states are considered when linting in isolation; when linting a `.psc` file dropped in the app, its `Extends` ancestry is resolved from the project root too. | |
| **Conflicting script versions** | Flags, as a `[warning]`, a `.psc` file when another script search directory contains a case-insensitively same-named file with different contents (determined by MD5), since which version Papyrus resolves can depend on search-directory order. Byte-identical copies are ignored. Only available when linting a file with project context in the desktop app or CLI. | |

### Bugprone

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Implicit Float-to-Int conversion** | Flags a Float value declared, assigned, returned, or passed as an argument into an Int-typed slot without an explicit `as Int` cast. | |
| **Unreachable statement** | Flags statements that follow a `Return` within the same block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a `While` body), since they can never execute. | |
| **Static condition** | Flags `If`/`ElseIf`/`While` conditions that fold to a constant `true` or `false` (e.g. `If true`, `If 1 == 2`, `If !false && 3 > 4`), regardless of any runtime state, as a `[warning]`. Only conditions built entirely from literals (combined with arithmetic, comparison, logical, and unary operators) are checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |
| **Division by zero** | Flags, as a `[warning]`, a `/` or `%` whose right-hand operand is a compile-time-constant zero (e.g. `x / 0`, `x % 0.0`, `x / (1 - 1)`), since that crashes the script at runtime. Only a divisor built entirely from literals (combined with arithmetic and unary operators) is checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |
| **Empty loop/conditional body** | Flags, as a `[warning]`, since this is almost always a forgotten piece of logic rather than something intentional: a `While` loop whose body is empty, or whose body only nudges a variable by a constant amount (`i += 1`, `i -= 1`, or the equivalent `i = i + 1`/`i = i - 1`) with nothing else giving the loop a purpose (a step built from anything but a literal, such as a call or another variable, is left alone since it has a side effect of its own); and an empty `If`, `ElseIf`, or `Else` body. An `Else` clause is told apart from no `Else` clause at all (both parse to an empty body) by scanning for a literal `Else` immediately followed by `EndIf` in the source. | |
| **None used as an existing Form** | Flags a member/method access (`a.GetName()`, `a.Name`) on a local variable or script-level `Auto`/`AutoReadOnly` property that's still known to be `None` (e.g. `Armor a = None` followed directly by `a.GetName()`), since that crashes the script at runtime. An object-typed local without an initializer, or an object-typed `Auto`/`AutoReadOnly` property with no explicit default value (or an explicit `= None`), starts out `None` in every function, since it may not be set until something outside the script (the CK's Property Manager, another script, `OnInit`, …) does so. From there, a variable/property is tracked as `None` from its declaration/assignment until it's reassigned something else, narrowing through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) and through a `While` loop's condition (this language has no `break`/`continue`, so the loop can only exit once its condition is false). A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Anything less direct is left unflagged rather than guessed at. | |
| **Local variable shadowing** | Flags a local variable (declared with `Type name = ...` inside a function/event) whose name matches (case-insensitively) a `Property` declared on the same script, since referencing that name inside the function then reads the local rather than the property. When linting a `.psc` file dropped in the app, a local that instead shadows a property declared on a parent script (resolved through `Extends`) is flagged too. | |
| **Parameter reassignment** | Flags, as a `[warning]`, a function/event parameter assigned a new value anywhere in its own body (`total = 1`, `total += 1`, ...), since reusing the parameter's name for a different value discards what the caller passed in and can confuse a reader expecting it to still reflect the original argument. A member/index assignment built from a parameter (`akRef.Foo = 1`), or a reassignment of an unrelated local variable, is never flagged. | |
| **Form parameter used without a None check** | Flags, as a `[warning]`, a member/method access (`akForm.GetName()`) on a `Form`-typed function parameter that hasn't yet been confirmed non-`None` in that path, since a caller can always pass in `None` and dereferencing it crashes the script at runtime. Tracks a parameter as unconfirmed from the start of its function until it's narrowed through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) or a `While` loop's condition, the same way "None used as an existing Form" above narrows its own state, or until it's reassigned to anything else. A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Passing the parameter on as an argument to another call isn't flagged, only a direct member/method access is. Disabled by default, since many scripts intentionally accept a possibly-`None` Form and defer the check to a caller or a later branch; a project opts in via `rules.unchecked_form_parameter`. | |
| **Unchecked cast** | Flags, as a `[warning]`, a member/method access on the result of an `as` cast (e.g. `(akRef as Actor).GetActorValue("Health")`) before that result has been checked against `None`, since a cast that doesn't match the underlying Form's actual type evaluates to `None` at runtime rather than raising an error, so dereferencing it immediately crashes the script. Tracks a local variable as an unchecked cast result from its declaration/assignment from an `as` expression until it's reassigned something else, clearing it the moment a direct `None` check on it (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) is evaluated, regardless of which branch is ultimately taken — this lint only cares whether the possibility of `None` was ever considered, not which branch handles it. A cast used directly inline (`(value as Type).Member`) is always flagged, since there's no way to check it in between. | |

### Other

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Unused script properties** | Flags `Property` declarations whose name is never referenced anywhere else in the script. | |
| **Cyclomatic complexity** | Flags functions/events whose cyclomatic complexity (1 plus each `If`/`ElseIf` branch, `While` loop, and short-circuiting `&&`/`\|\|` operator) exceeds a configurable threshold, as a `[warning]` above `cyclomatic_complexity_warning` (default 10) or an `[error]` above `cyclomatic_complexity_error` (default 20). | |
| **Unused or write-only local variables** | Flags a local variable (declared with `Type name = ...` inside a function/event) whose value is never read: either it's never referenced again at all, or it's only ever reassigned (`name = ...`) without that new value ever being read back. Reading a variable via a compound assignment (`name += ...`, etc.) or through a member/index expression built from it (`name.Foo`, `name[0]`) counts as a use. Function parameters and script properties aren't locals and are never flagged by this lint. | |
| **Prefer named arguments** | Flags, as a `[warning]`, a positional call argument that the configured `named_arguments` setting prefers to see passed by Papyrus's named-argument syntax instead (`func(argB = 1)`): `always` flags every positional argument, `instead_of_defaults` flags only an argument filling a parameter that has a default value, and `never` (the default) flags nothing. Parameter names and default values are only known for functions declared in the script being linted (including via `self.Func(...)`), so a call to a function declared on another script is never flagged. An argument already passed by name is always accepted regardless of setting. | |
| **Useless downcast** | Flags, as an `[info]`, an explicit `as` cast that can't actually narrow anything: either its target type exactly matches the value's already-known type, or the value's type already extends the target (directly or transitively) — e.g. `Actor dude` followed by `Foo(dude as ObjectReference)`, since `Actor` already extends `ObjectReference` and Papyrus would accept `dude` there without the cast. Only a cast whose value's type can be determined locally (locals, parameters, properties, `Self`/`Parent`, literals, and other resolvable expressions) is checked; a member access or function call result is left unflagged rather than guessed at. Primitive types (`Int`, `Float`, `Bool`, `String`) are only flagged for an exact-type cast, never treated as extending one another, so a meaningful conversion like an explicit `Int`-to-`Float` widening cast is never flagged. When linting a `.psc` file dropped in the app, a cast target that's an ancestor of the value's script (rather than an exact match) is resolved the same way the argument/return type checks resolve their own, including through the native engine type fallback for types like `Actor`/`ObjectReference`/`Form`. | |
| **Unused disable directive** | Flags, as a `[warning]`, each rule id in an `@disable` comment that is unknown or does not suppress a diagnostic from that rule on its line. A bare `@disable` is flagged when its line has no diagnostics to suppress. Disabled by default; opt in with `rules.unused_disable`. | |

The formatting lints/fixes (trailing whitespace, space after comma,
semicolon, indentation, chain whitespace, exclamation mark spacing, and
operator spacing) never flag or change a line inside a
CreationKit-generated `;BEGIN FRAGMENT CODE`/`;END FRAGMENT CODE` block,
except the actual script code between a `;BEGIN CODE`/`;END CODE` pair
within it. Reformatting the rest of that block (fragment headers, the
generated function signature, `EndFunction`, or the markers themselves)
would make CreationKit fail to recognize the fragment.

## Disabling a lint on a specific line

A line carrying a trailing `; @disable <rule-id>[, <rule-id>...]` comment
has diagnostics from the named rule(s) suppressed for that line only, e.g.:

```papyrus
action = 1 ; @disable float-to-int
```

`; @disable` with no rule ids suppresses every lint on that line. Matching
against the directive's rule id(s) is case-insensitive. This only affects
linting — it does not change what automatic fixes do to that line. The rule ids, one per
lint listed above, are: `trailing-whitespace`, `comma-spacing`,
`forbidden-functions`, `slow-functions`, `unused-getter`, `unused-property`,
`semicolon`, `float-to-int`, `strict-boolean`, `argument-types`,
`return-types`, `function-override`, `argument-naming`, `numeric-comparison`,
`indentation`, `cyclomatic-complexity`, `unreachable-statement`,
`static-condition`, `division-by-zero`, `unused-local-variable`, `none-form-usage`,
`local-variable-shadowing`, `parameter-reassignment`, `chain-whitespace`, `exclamation-spacing`,
`identifier-casing`, `type-casing`, `named-arguments`, `operator-spacing`,
`property-sorting`, `explicit-return`, `unchecked-form-parameter`,
`unchecked-cast`, `unresolved-script`, `short-wait-interval`,
`state-function-signature`, `goto-state`, `conflicting-script-versions`, and
`unused-disable`.

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed at the project root:
next to the `.achlist` file you drop into the app, or, for a single
`.psc` file dropped directly, two directories above it (e.g. `Data` for
`Data/Scripts/Source/abc.psc`). Any key it omits falls back to its
default:

```yaml
# Path to PapyrusCompiler.exe, or null to auto-detect it
compiler_path: null
# Extra directories (relative to the project root, or absolute) to search
# for .psc files, besides scripts/source and source/scripts
additional_script_roots: []
# true, false; also runs PapyrusCompiler.exe (into a throwaway temporary
# directory) as part of linting a dropped .psc, reporting its errors
# alongside the lint engine's own. Requires compiler_path to be set/
# auto-detected
compile_check: false
# true, false
semicolon: false
# tab, space
indentation: tab
# Non-negative integer; used only when indentation is space
indentation_width: 4
# camelCase, PascalCase, snake_case, CONSTANT_CASE
identifier_casing: PascalCase
# Non-negative integer
cyclomatic_complexity_warning: 10
# Non-negative integer
cyclomatic_complexity_error: 20
# PascalCase, camelCase, lowercase, UPPERCASE
type_casing: PascalCase
# always, instead_of_defaults, never
named_arguments: never
# Non-negative number
min_wait_interval: 0.1
# true, false
fail_on_warning: false
# true, false
fail_on_info: false
# Each rule accepts true or false
rules:
  trailing_whitespace: true
  comma_spacing: true
  forbidden_functions: true
  slow_functions: true
  unused_getter: true
  unused_property: true
  semicolon: true
  float_int_conversion: true
  strict_boolean: true
  argument_types: true
  return_types: true
  function_override: true
  argument_naming: true
  numeric_comparison: true
  indentation: true
  cyclomatic_complexity: true
  unreachable_statement: true
  static_condition: true
  division_by_zero: true
  unused_local_variable: true
  none_form_usage: true
  local_variable_shadowing: true
  parameter_reassignment: true
  chain_whitespace: true
  exclamation_spacing: true
  identifier_casing: true
  type_casing: true
  named_arguments: true
  operator_spacing: true
  property_sorting: false
  explicit_return: true
  unchecked_form_parameter: false
  unchecked_cast: true
  unresolved_script: true
  short_wait_interval: true
  state_function_signature: true
  goto_state: true
  too_many_states: true
  multiple_auto_states: true
  conflicting_script_versions: true
  unused_disable: false
```

- `compiler_path`: an explicit path to `PapyrusCompiler.exe`, set via the
  app's Settings tab. When unset (or blank), the app auto-detects it at
  `PapyrusCompiler.exe` inside a `Papyrus Compiler` directory one level
  above the project's `.achlist` directory (the layout used by Bethesda's
  Creation Kit tooling, where a game's `Data` directory sits alongside a
  `Papyrus Compiler` directory in the game's install root).
- `additional_script_roots`: extra directories, besides the conventional
  `scripts/source` and `source/scripts` under the project root, to search
  for `.psc` files — set via the app's Settings tab, one per line. Each
  entry is resolved relative to the project root unless it's already an
  absolute path. Searched (after the two conventional directories, in the
  order listed) when resolving cross-script lookups for the "Argument type
  check"/"Return type check"/"Function override" lints and autocompletion,
  and appended to the compiler's `-i` argument (see Compiling a script
  below) — useful when a script imports from a shared library location
  outside the project. The CLI also accepts one or more `--script-root
  <path>` flags on top of this setting (see Command-line interface below).
- `compile_check`: whether the desktop app also runs PapyrusCompiler.exe
  against a dropped `.psc` as part of linting it — set via the app's
  Settings tab, alongside `compiler_path`. `false` by default, since it's
  slower than the lint engine's own, dependency-free checks and requires a
  configured compiler path. When enabled, PapyrusCompiler.exe's own
  reported errors (e.g. a syntax mistake the lint engine's more forgiving
  parser lets through) are added to the results as `[error]` diagnostics,
  the same way the app's other lints are. Compiles into a throwaway
  temporary directory rather than the project's real output directory, so
  enabling this never touches (or requires write access to) the project's
  actual compiled `.pex` output — see Compiling a script below.
- `semicolon`: whether lines are required to end in a semicolon (`true`)
  or must not (`false`). Read by the "Semicolon at end of line" lint/fix.
- `indentation`: the expected indentation style, `tab` or `space`. Read
  by the "Formatting checks" lint and the indentation automatic fix.
- `indentation_width`: the number of spaces per indentation level, used
  only when `indentation` is `space`.
- `identifier_casing`: the casing style declared identifiers must match:
  `camelCase`, `PascalCase`, `snake_case`, or `CONSTANT_CASE`. Read by the
  "Identifier casing" lint.
- `cyclomatic_complexity_warning` / `cyclomatic_complexity_error`: the
  cyclomatic complexity a function/event can reach before the
  "Cyclomatic complexity" lint flags it as a `[warning]` or an `[error]`,
  respectively.
- `type_casing`: the casing convention required of a script's declared
  type name (the identifier following `ScriptName`), one of `PascalCase`,
  `camelCase`, `lowercase`, or `UPPERCASE`. Read by the "Type name casing"
  lint.
- `named_arguments`: how strongly a positional call argument should be
  passed by name instead, one of `always`, `instead_of_defaults`, or
  `never` (the default). Read by the "Prefer named arguments" lint.
- `min_wait_interval`: the interval/duration argument a `Utility.Wait`,
  `RegisterForUpdate`, `RegisterForSingleUpdate`,
  `RegisterForUpdateGameTime`, or `RegisterForSingleUpdateGameTime` call
  can go below before the "Short wait/update interval" lint flags it as a
  `[warning]`. Defaults to `0.1`.
- `fail_on_warning` / `fail_on_info`: whether the command-line interface
  (see below) treats a `[warning]`-level or `[info]`-level diagnostic,
  respectively, as a reason to exit non-zero. Both default to `false`, so
  by default only `[error]`-level diagnostics fail a CLI run;
  `[warning]`/`[info]`-level diagnostics are still printed either way. Has
  no effect on the desktop app, which always lists every diagnostic
  regardless of severity.
- `rules`: per-lint enable/disable switches. Setting one to `false` turns
  that lint (and its automatic fix, if it has one) off entirely; every
  key under `rules` can be omitted individually and falls back to its
  default. Every key defaults to `true` except `property_sorting` and
  `unchecked_form_parameter`, and `unused_disable`, which default to `false`:
  reordering a script's declared properties is a more invasive change than
  the rest of these lints, many scripts intentionally accept a possibly-`None`
  Form and defer the check to a caller or a later branch, and reporting stale
  suppressions is opt-in to avoid surprising existing projects. The key names
  match the lints listed above: `trailing_whitespace`, `comma_spacing`,
  `forbidden_functions`, `slow_functions`, `unused_getter`, `unused_property`,
  `semicolon`, `float_int_conversion`, `strict_boolean`,
  `argument_types`, `return_types`, `function_override`, `numeric_comparison`,
  `indentation`, `cyclomatic_complexity`, `unreachable_statement`,
  `static_condition`, `division_by_zero`, `unused_local_variable`, `none_form_usage`,
  `local_variable_shadowing`, `chain_whitespace`, `exclamation_spacing`,
  `identifier_casing`, `type_casing`, `named_arguments`, `operator_spacing`,
  `property_sorting`, `explicit_return`, `unchecked_form_parameter`,
  `unchecked_cast`, `unresolved_script`, and `short_wait_interval`.

The app's formatting controls (trailing semicolons, indentation style,
indentation width) are backed by this file: on startup it reads the
config file for the most recently opened project and pre-selects those
controls accordingly, and any change made to them is written straight
back to the file, so the project's formatting settings persist between
sessions and can be shared/committed alongside the project. The
PapyrusCompiler.exe path field on the Settings tab works the same way,
except it's pre-filled with an auto-detected path (see `compiler_path`
above) rather than a fixed default when the project has no explicit
override saved yet. The additional script roots textarea (see
`additional_script_roots` above) works the same way too, one directory per
line.

## Command-line interface

Besides its GUI, Papyrus Lint can lint non-interactively from the
command line two ways: by passing an `.achlist` (or a single `.psc`) path
to the desktop app's own executable (`PapyrusLinter`), or via the
standalone `PapyrusLinterCLI` binary (`app/crates/papyrus-lint-cli`) built and
shipped separately for use cases — e.g. a CI pipeline — that shouldn't
need the desktop app's binary (and its GUI dependencies) at all. Both
accept the same argument and behave identically:

```text
PapyrusLinterCLI path/to/project.achlist
PapyrusLinterCLI init
PapyrusLinterCLI path/to/Example.psc
PapyrusLinterCLI fix path/to/project.achlist
PapyrusLinterCLI fix path/to/Example.psc
PapyrusLinterCLI --json path/to/project.achlist
PapyrusLinterCLI --json fix path/to/project.achlist
PapyrusLinterCLI --config path/to/papyrus-lint.yaml path/to/Example.psc
PapyrusLinterCLI --script-root path/to/SharedScripts path/to/project.achlist
PapyrusLinterCLI --output path/to/report.txt path/to/project.achlist
PapyrusLinterCLI --json --output path/to/report.json path/to/project.achlist
```

`PapyrusLinterCLI init` creates a `papyrus-lint.yaml` containing all default
settings in the current working directory. It refuses to overwrite an existing
`papyrus-lint.yaml` or `papyrus-lint.yml` file.

Given an `.achlist` path, it resolves every `.psc` entry listed in it.
Given a single `.psc` path directly, it lints just that file, treating it
as the achlist's sole entry. Either way, each script is linted against
the project's `papyrus-lint.yaml`/`.yml` config file (see Configuration
above). For an `.achlist`, the project root is its containing directory; for
a bare `.psc`, the root is found by walking up from the file for a
`Scripts/Source` or `Source/Scripts` directory pair (matched
case-insensitively) and taking the directory above it — so it's found
correctly even for a script nested further still, e.g. a namespaced
`Scripts/Source/User/MyScript.psc`, not just the conventional two
directories up. If no config exists there, the documented defaults apply.
Each diagnostic found is printed as `<path>:<line>:<column>: [<rule>]
<message>`, followed by a one-line summary. Calls to functions declared on
other scripts under the project root are resolved the same way the
desktop app resolves them, so the CLI's "Argument type check"/"Return type
check" results match what dropping the same `.achlist` into the app would
report.

Given `--config <path>` (combinable with `fix`/`--json`, in any argument
order), the CLI loads lint configuration directly from `<path>` instead
of discovering `papyrus-lint.yaml`/`.yml` from the project root — useful
when a config file lives somewhere other than that project root, or isn't
named `papyrus-lint.yaml`/`.yml`. Both editor plugins expose this as a
`config_path`/`configPath` setting (see their own READMEs). Since the
project root's own config file is bypassed entirely in that case, so is
its `additional_script_roots`; use `--script-root` (below) to add any back
explicitly.

Given one or more `--script-root <path>` flags (combinable with
`fix`/`--json`/`--config`, in any argument order), each given directory
(resolved relative to the project root unless already absolute) is
searched for `.psc` files alongside `scripts/source`/`source/scripts` and
the project's configured `additional_script_roots` (see Configuration
above) — letting a caller add a script root for a single run without
editing the project's config file.

Given `--output <path>` (combinable with `fix`/`--json`/`--config`/
`--script-root`, in any argument order), the report — plain text or JSON,
whichever `--json` selects — is written to `<path>` instead of stdout, so
it can be stored directly without piping the command's output to a file.
Usage/error text still goes to stderr either way, and the exit status is
unaffected.

Each `.psc` file is decoded as UTF-8 when it's valid UTF-8, or as
Windows-1252 (CP1252) — the Creation Kit/Papyrus compiler's own default
encoding for the language — otherwise, so a script saved in either
encoding lints correctly instead of aborting the whole run.

Prefixed with the `fix` subcommand, it applies every automatic fix (the
"Auto-Fix" lints in the table above, using the same config's semicolon and
indentation settings) to each resolved script first, rewriting a script on
disk only if it changed, before reporting whatever diagnostics remain the
same way — the same repair the desktop app's "Fix" button applies to a
single script.

Given the `--json` flag (combinable with `fix`, in either argument order),
the CLI prints a single JSON document to stdout instead of the plain-text
lines and summary, so editor plugins and other tooling can consume the
report without scraping text. The output contract is published as a
[JSON Schema](papyrus-lint-report.schema.json) using JSON Schema Draft 2020-12,
so integrations can generate types and validate saved or streamed reports:

```console
PapyrusLinterCLI --json --output report.json path/to/project.achlist
npx ajv-cli validate --spec=draft2020 -s papyrus-lint-report.schema.json -d report.json
```

An example valid report is:

```json
{
  "files": [
    {
      "path": "scripts/source/Example.psc",
      "diagnostics": [
        { "line": 3, "column": 1, "rule": "trailing-whitespace", "level": "warning", "message": "[warning] Line contains trailing whitespace" }
      ]
    }
  ],
  "scripts_checked": 1,
  "files_with_diagnostics": 1,
  "total_diagnostics": 1,
  "files_fixed": null,
  "success": true
}
```

Every resolved script gets a `files` entry, even one with no diagnostics,
so a consumer can clear stale diagnostics for a file that's since become
clean. `level` is always `"error"`, `"warning"`, or `"info"`. Every built-in lint
sets a level; an untagged external diagnostic is conservatively reported as
`"error"` — see `Diagnostic::level`. `files_fixed` is
only present (non-`null`) when run with the `fix` subcommand. `success`
reports whether the run would exit `0`; here it's `true` because a
`[warning]`-level diagnostic doesn't fail the run under the default
`fail_on_warning: false`.

It exits `0` if no diagnostics were found (or none of the ones found
counted as a failure — see `fail_on_warning`/`fail_on_info` under
Configuration above), `1` if any did, or `2` on a usage error (a
missing/extra argument) or an I/O error (the `.achlist` or `.psc` file, or
one being fixed, couldn't be read/written/parsed) — so it can gate a CI
step on a clean lint run.

Run it with `--version`/`-V` to print its version (`PapyrusLinterCLI
<version>`) and exit `0` instead of linting; the desktop app shows its own
version next to its title.

Launched with no arguments, the desktop app's own executable starts its
GUI as normal; launched with any arguments, including an `.achlist` or `.psc`
path (or `-h`/`--help`), it routes all of them through the same CLI
implementation described above. Windows release builds use the console
subsystem so shells wait for CLI-mode completion and can reliably capture
plain-text or JSON stdout and stderr; when launched without arguments, the
executable detaches that console before starting the GUI.

Prebuilt `PapyrusLinterCLI`/`PapyrusLinterCLI.exe` standalone CLI binaries
for Linux, macOS, and Windows are attached to each [GitHub
release](https://github.com/Idrinth/papyrus-lint/releases), alongside the
desktop app's own bundles. To build the standalone CLI yourself instead,
run `cargo build --release --manifest-path
app/crates/papyrus-lint-cli/Cargo.toml`; the resulting binary is named
`PapyrusLinterCLI`.

Each release also attaches a packaged copy of the two editor plugins: the
VS Code extension as a `.vsix` file (install via "Install from VSIX..." in
VS Code, or `code --install-extension <file>`) and the SublimeLinter
plugin as a `.zip` of the `SublimeLinter-contrib-papyrus-lint` directory
(extract it into Sublime Text's `Packages/` directory). Both still depend
on the standalone `PapyrusLinterCLI` binary above being installed and on
`PATH` (or configured via each plugin's settings).

## Compiling a script

Each `.psc` file listed on the Lint results tab has a "Compile" button that
recompiles it with `PapyrusCompiler.exe` (see `compiler_path` under
Configuration above for how that executable's path is resolved), so a fix
made in the code viewer can be tried out without leaving the app. The code
viewer's editor has the same capability built in: alongside "Save" and
"Cancel", a "Save & Compile" button writes the edited script to disk and
then immediately recompiles it, showing the result beneath the editor —
so a fix can be saved and verified in one step, without reopening the
file from the Lint results list. Either button runs:

```text
PapyrusCompiler.exe "<script path>" -i="<source dir 1>;<source dir 2>" -o="<output dir>" -f="TESV_Papyrus_Flags.flg"
```

where `<script path>` is the path to the `.psc` file, whose parent directory
is the source directory
(conventionally a `scripts/source` or `source/scripts` directory under the
project root) and `<output dir>` is its parent, matching the layout
Bethesda's tooling expects — a `Source` directory holding `.psc` files
inside the `Scripts` directory that receives the compiled `.pex` output.
`-i` is given both of those conventional source directories under the
project root, plus any configured `additional_script_roots` (see
Configuration above), separated by `;` (PapyrusCompiler.exe accepts
multiple import directories that way), so the script can still resolve
imports from the other layout, or a configured additional root, even
though it only lives in one of them.
The compiler is run with its containing directory as the working directory,
allowing it to resolve the bundled `TESV_Papyrus_Flags.flg` passed by the final
`-f` argument.

The compiler's stdout/stderr is shown once it finishes (beneath the
"Compile" button on the Lint results list, or beneath the editor for
"Save & Compile"), styled green on success and red on failure, so both a
successful compile and a reported error (a syntax error, a missing
import, etc.) are visible without checking a log file. If no compiler
path is configured or auto-detected, or the executable itself can't be
run, that's reported the same way rather than silently doing nothing.

`PapyrusCompiler.exe` embeds the compiling machine's Windows username and
computer name into every `.pex` it writes, right next to the source file
name in its header. On a successful compile, Papyrus Lint reads that
header back out of the resulting `.pex` and blanks both fields in place,
so a script compiled locally and then shared (e.g. bundled into a mod)
doesn't leak who built it or what machine they built it on. A note is
added to the compile output when this happens.

Enabling `compile_check` (see Configuration above) also runs
PapyrusCompiler.exe as part of linting a dropped `.psc` — automatically,
not just from the "Compile"/"Save & Compile" buttons — and reports any
errors it finds as `[error]` diagnostics alongside the lint engine's own,
so a syntax mistake the compiler itself rejects (but the lint engine's
own, more forgiving parser doesn't) still shows up in the results. Unlike
the "Compile" button above, this always compiles into a throwaway
temporary directory rather than the project's real `Scripts` output
directory, so it never overwrites (or requires write access to) the
project's actual compiled `.pex` output, and never needs the
personal-data stripping described above — the compiled output is
discarded either way.
