# Papyrus Lint [![Quality gate status](https://sonarcloud.io/api/project_badges/measure?project=Idrinth_papyrus-lint&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=Idrinth_papyrus-lint)

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
| **Identifier casing** | Flags a declared function/event, property, state, parameter, or local/script variable whose name doesn't match the configured `identifier_casing` style: `camelCase`, `PascalCase`, `snake_case`, or `CONSTANT_CASE`. `ScriptName` itself is never checked by this lint (see "Type name casing" below). A parameter has no location of its own, so it's reported on its enclosing function's line. | |
| **Type name casing** | Flags, as a `[warning]`, a script's declared type name (the identifier following `ScriptName`) if it doesn't follow the configured `type_casing` convention (`PascalCase`, `camelCase`, `lowercase`, or `UPPERCASE`). Only the script's own declared name is checked, never its `Extends` target, since that type is declared (and presumably already checked) in another script. Note that a script's `ScriptName` must match its `.psc` filename case-insensitively, so a substantive rename to satisfy this lint means renaming the file too — a case-only change does not. | |
| **Spacing around logical/comparison operators** | Requires, as a `[warning]`, exactly one space on either side of `&&`, `\|\|`, `==`, `!=`, `>`, `<`, `>=`, and `<=`. A side whose whitespace reaches a newline (the operator opens or closes a statement continued across physical lines) is left unchecked on that side. The fix normalizes each flagged side to a single space, without reaching across a newline. | ✓ |
| **Property sorting** | Flags, as a `[warning]`, a `Property` declaration that isn't sorted by type and then alphabetically by name, or that isn't declared immediately after the `ScriptName` line, before any variable, function, or state declaration (an `Import` isn't tracked closely enough to count against this). Disabled by default, since reordering a script's declared properties is a more invasive change than the rest of these lints; a project opts in via `rules.property_sorting`. The fix relocates each property's own declaration lines (its full `Property`/`EndProperty` block, for a non-auto property) as a group right after `ScriptName`, in sorted order; a documentation comment placed directly above a property is left behind rather than moved with it. | ✓ |

### Performance

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Forbidden/discouraged function usage** | Flags calls to functions listed in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls), with a configurable severity and an explanatory message per entry. | |
| **Slow function usage** | Flags calls to functions listed in `rules/slow-functions.yaml` that have a faster equivalent available, and suggests the quicker alternative. | |

### Reliability

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Getter usage without saving result** | Flags standalone calls to functions whose names begin with `Get` (case-insensitively), because their return value is discarded. | |
| **Strict boolean check** | Flags `If`/`ElseIf`/`While` conditions that aren't already a `Bool` value or expression, instead of relying on Papyrus's implicit conversion to boolean. Only conditions whose type can be determined locally (locals, parameters, properties, literals, casts, and comparison/logical expressions) are checked; a condition that depends on a function call or a member access is left unflagged rather than risk a false positive. | |
| **Argument type check** | Flags call-site arguments whose type doesn't match the callee's declared parameter type (e.g. passing a `String` where an `Int` is expected), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as passing an object whose script extends (directly or transitively) the parameter's type (e.g. passing an `Armor` where a `Form` is expected, or an `Actor` where an `ObjectReference` is expected). Calls to functions declared in the same script are always checked; when linting a `.psc` file dropped in the app, calls to functions declared on other scripts under the project root (e.g. `SomeProperty.DoThing(...)`) are checked too, by resolving those scripts' signatures (including through `Extends`), and the `Extends` chain of an argument's own script is likewise resolved from the project root to allow compatible subtypes. Native engine types (e.g. `Actor`, `ObjectReference`, `Form`, `Spell`) whose own `.psc` isn't part of the project fall back to the common Skyrim/Fallout 4 native class hierarchy listed in `rules/native-types.yaml`, so a subtype relationship between them is still recognized. A call whose target or argument type can't be determined is skipped rather than guessed at. | |
| **Return type check** | Flags `Return` statements whose value's type doesn't match the enclosing function's declared return type (e.g. returning a `String` from a Function declared `Int`), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as returning an object whose script extends (directly or transitively) the declared return type (e.g. returning an `Armor` from a Function declared `Form`, or an `Actor` from a Function declared `ObjectReference`). When linting a `.psc` file dropped in the app, a returned value's own script's `Extends` chain is resolved from the project root to allow compatible subtypes there too, with the same native-type fallback used by the argument type check for engine types like `Actor`/`ObjectReference`/`Form`. A `Return` whose value's type can't be determined, or with no declared return type, is skipped rather than guessed at. | |
| **Inherited function override** | Flags, as an `[info]`, a function declared on this script that shares its name with a function declared on the script it `Extends` (directly or transitively) — the local declaration silently replaces the inherited one. This is often intentional (e.g. overriding an `Event OnInit()` handler), so it's informational rather than a warning. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked (state-based override is a separate mechanism from `Extends`). | |
| **Argument naming consistency** | Flags, as a `[warning]`, a function declared on this script whose parameter name doesn't match (case-insensitively) the corresponding parameter of the same-named function declared on the script it `Extends` (directly or transitively) — since Papyrus resolves a named-argument call against the declared type of the reference it's called through, a renamed parameter on an override can silently misdirect (or fail to compile) a caller using the parent's names. Only checked when linting a `.psc` file dropped in the app, by resolving the `Extends` chain from the project root; a function declared inside a `State` block is not checked, and only parameter positions present on both declarations are compared. | |
| **Strict numeric type check** | Flags implicit comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) between an `Int` value and a `Float` value without an explicit cast making the comparison exact. Only comparisons whose operand types can be determined locally are checked. | |
| **Explicit return on every path** | Flags, as an `[error]`, a typed function/event with a code path that falls off the end of its body without a `Return`, since Papyrus then silently returns that type's default value (`0`, `""`, `False`, or `None`) instead of one the author chose. A `Return` with no value still counts as long as it's reached (`return_types` covers a value's actual type); an `If` only counts when every branch, including an `Else`, returns, and a `While` loop is never assumed to guarantee one since it may run zero times. A native function has no body to inspect and is never flagged. | |
| **Unresolved script reference** | Flags, as a `[warning]`, a call through Papyrus's static/global call syntax (e.g. `MyMissingScript.DoThing()`) whose target script can't be found, since a call through a script that doesn't exist can never compile. Only a call whose object is a bare identifier not already known as a local variable, parameter, or property is considered a script reference at all — one resolved through a variable or property is left to the "Argument type check"/"Return type check" lints instead. Only checked when linting a `.psc` file dropped in the app, by resolving the name against `.psc` files under the project root the same way the argument/return type checks do, falling back to a list of common native singleton scripts (`Game`, `Utility`, `Debug`, `Math`, `StringUtil`, `Input`, `UI`, `StorageUtil`) in `rules/native-globals.yaml` that the game ships compiled with no project-side source. | |

### Bugprone

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Implicit Float-to-Int conversion** | Flags a Float value declared, assigned, returned, or passed as an argument into an Int-typed slot without an explicit `as Int` cast. | |
| **Unreachable statement** | Flags statements that follow a `Return` within the same block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a `While` body), since they can never execute. | |
| **Static condition** | Flags `If`/`ElseIf`/`While` conditions that fold to a constant `true` or `false` (e.g. `If true`, `If 1 == 2`, `If !false && 3 > 4`), regardless of any runtime state, as a `[warning]`. Only conditions built entirely from literals (combined with arithmetic, comparison, logical, and unary operators) are checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |
| **Division by zero** | Flags, as a `[warning]`, a `/` or `%` whose right-hand operand is a compile-time-constant zero (e.g. `x / 0`, `x % 0.0`, `x / (1 - 1)`), since that crashes the script at runtime. Only a divisor built entirely from literals (combined with arithmetic and unary operators) is checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |
| **None used as an existing Form** | Flags a member/method access (`a.GetName()`, `a.Name`) on a local variable or script-level `Auto`/`AutoReadOnly` property that's still known to be `None` (e.g. `Armor a = None` followed directly by `a.GetName()`), since that crashes the script at runtime. An object-typed local without an initializer, or an object-typed `Auto`/`AutoReadOnly` property with no explicit default value (or an explicit `= None`), starts out `None` in every function, since it may not be set until something outside the script (the CK's Property Manager, another script, `OnInit`, …) does so. From there, a variable/property is tracked as `None` from its declaration/assignment until it's reassigned something else, narrowing through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) and through a `While` loop's condition (this language has no `break`/`continue`, so the loop can only exit once its condition is false). A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Anything less direct is left unflagged rather than guessed at. | |
| **Local variable shadowing** | Flags a local variable (declared with `Type name = ...` inside a function/event) whose name matches (case-insensitively) a `Property` declared on the same script, since referencing that name inside the function then reads the local rather than the property. When linting a `.psc` file dropped in the app, a local that instead shadows a property declared on a parent script (resolved through `Extends`) is flagged too. | |
| **Form parameter used without a None check** | Flags, as a `[warning]`, a member/method access (`akForm.GetName()`) on a `Form`-typed function parameter that hasn't yet been confirmed non-`None` in that path, since a caller can always pass in `None` and dereferencing it crashes the script at runtime. Tracks a parameter as unconfirmed from the start of its function until it's narrowed through `If`/`ElseIf`/`Else` branches guarded by a direct `None` check (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) or a `While` loop's condition, the same way "None used as an existing Form" above narrows its own state, or until it's reassigned to anything else. A branch that unconditionally `Return`s doesn't carry its state past the `If`, covering the common `If x == None` / `Return` guard idiom. Passing the parameter on as an argument to another call isn't flagged, only a direct member/method access is. Disabled by default, since many scripts intentionally accept a possibly-`None` Form and defer the check to a caller or a later branch; a project opts in via `rules.unchecked_form_parameter`. | |
| **Unchecked cast** | Flags, as a `[warning]`, a member/method access on the result of an `as` cast (e.g. `(akRef as Actor).GetActorValue("Health")`) before that result has been checked against `None`, since a cast that doesn't match the underlying Form's actual type evaluates to `None` at runtime rather than raising an error, so dereferencing it immediately crashes the script. Tracks a local variable as an unchecked cast result from its declaration/assignment from an `as` expression until it's reassigned something else, clearing it the moment a direct `None` check on it (`x == None`, `x != None`, `!x`, a bare `x`, optionally combined with `&&`/`\|\|`) is evaluated, regardless of which branch is ultimately taken — this lint only cares whether the possibility of `None` was ever considered, not which branch handles it. A cast used directly inline (`(value as Type).Member`) is always flagged, since there's no way to check it in between. | |

### Other

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Unused script properties** | Flags `Property` declarations whose name is never referenced anywhere else in the script. | |
| **Cyclomatic complexity** | Flags functions/events whose cyclomatic complexity (1 plus each `If`/`ElseIf` branch, `While` loop, and short-circuiting `&&`/`\|\|` operator) exceeds a configurable threshold, as a `[warning]` above `cyclomatic_complexity_warning` (default 10) or an `[error]` above `cyclomatic_complexity_error` (default 20). | |
| **Unused or write-only local variables** | Flags a local variable (declared with `Type name = ...` inside a function/event) whose value is never read: either it's never referenced again at all, or it's only ever reassigned (`name = ...`) without that new value ever being read back. Reading a variable via a compound assignment (`name += ...`, etc.) or through a member/index expression built from it (`name.Foo`, `name[0]`) counts as a use. Function parameters and script properties aren't locals and are never flagged by this lint. | |
| **Prefer named arguments** | Flags, as a `[warning]`, a positional call argument that the configured `named_arguments` setting prefers to see passed by Papyrus's named-argument syntax instead (`func(argB = 1)`): `always` flags every positional argument, `instead_of_defaults` flags only an argument filling a parameter that has a default value, and `never` (the default) flags nothing. Parameter names and default values are only known for functions declared in the script being linted (including via `self.Func(...)`), so a call to a function declared on another script is never flagged. An argument already passed by name is always accepted regardless of setting. | |

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
`local-variable-shadowing`, `chain-whitespace`, `exclamation-spacing`,
`identifier-casing`, `type-casing`, `named-arguments`, `operator-spacing`,
`property-sorting`, `explicit-return`, `unchecked-form-parameter`,
`unchecked-cast`, and `unresolved-script`.

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed next to the
`.achlist` file you drop into the app. Any key it omits falls back to
its default:

```yaml
# Path to PapyrusCompiler.exe, or null to auto-detect it
compiler_path: null
# Extra directories (relative to the project root, or absolute) to search
# for .psc files, besides scripts/source and source/scripts
additional_script_roots: []
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
  `unchecked_form_parameter`, which default to `false`: reordering a
  script's declared properties is a more invasive change than the rest of
  these lints, and many scripts intentionally accept a possibly-`None`
  Form and defer the check to a caller or a later branch. The key names
  match the lints listed above: `trailing_whitespace`, `comma_spacing`,
  `forbidden_functions`, `slow_functions`, `unused_getter`, `unused_property`,
  `semicolon`, `float_int_conversion`, `strict_boolean`,
  `argument_types`, `return_types`, `function_override`, `numeric_comparison`,
  `indentation`, `cyclomatic_complexity`, `unreachable_statement`,
  `static_condition`, `division_by_zero`, `unused_local_variable`, `none_form_usage`,
  `local_variable_shadowing`, `chain_whitespace`, `exclamation_spacing`,
  `identifier_casing`, `type_casing`, `named_arguments`, `operator_spacing`,
  `property_sorting`, `explicit_return`, `unchecked_form_parameter`,
  `unchecked_cast`, and `unresolved_script`.

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

```
PapyrusLinterCLI path/to/project.achlist
PapyrusLinterCLI path/to/Example.psc
PapyrusLinterCLI fix path/to/project.achlist
PapyrusLinterCLI fix path/to/Example.psc
PapyrusLinterCLI --json path/to/project.achlist
PapyrusLinterCLI --json fix path/to/project.achlist
PapyrusLinterCLI --config path/to/papyrus-lint.yaml path/to/Example.psc
PapyrusLinterCLI --script-root path/to/SharedScripts path/to/project.achlist
```

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
report without scraping text:

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
clean. `level` is `"error"`, `"warning"`, `"info"`, or (in practice, never
from a built-in lint) `null` — see `Diagnostic::level`. `files_fixed` is
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
GUI as normal; launched with an `.achlist` or `.psc` path (or
`-h`/`--help`), it lints from the command line instead, exactly as
described above. On
Windows release builds the desktop executable is compiled without a
console, so its CLI mode there is best-effort — the standalone CLI binary
below is the reliable way to lint from a Windows console or script.

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

```
PapyrusCompiler.exe "<source dir>" -f="<script name>.psc" -i="<source dir 1>;<source dir 2>" -o="<output dir>"
```

where `<source dir>` is the directory the `.psc` file lives in
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
