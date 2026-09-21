# Annotations

Papyrus Lint recognizes annotations inside Papyrus line comments. They add
linting metadata without changing how the Creation Kit compiles or runs the
script. Annotation names are case-insensitive.

## Function annotations

Place `@deprecated` and `@nodiscard` either in the trailing line comment on a
function header or in a line comment immediately above the header. They also
work with functions declared inside states and with backslash-continued
headers.

### `@deprecated [replacement info]`

Marks a project function as deprecated. The `deprecated-functions` rule warns
when that function is called, including calls found through another project
script's declarations.

```papyrus
; @deprecated Use OpenInventory() instead.
Function ShowInventory()
EndFunction
```

Replacement information after `@deprecated` is free-form documentation for
readers; when present, it is included in the diagnostic message. Keep the
annotation as a separate word: names such as `@deprecatedSoon` are not
recognized.

### `@nodiscard`

Marks a function whose return value should be used. The `unused-nodiscard`
rule warns when a call's result is discarded rather than assigned, returned,
or used by a surrounding expression such as a condition.

```papyrus
Int Function FindInventoryCount() ; @nodiscard
    Return 0
EndFunction
```

Papyrus Lint can follow this metadata across project scripts. This annotation
is separate from the `unused-getter` rule: a `Get*` function marked
`@nodiscard` can produce findings from both rules.

## Suppression annotations

Suppression annotations accept comma-separated [lint rule
IDs](https://papyrus-lint.idrinth.de/rules.html). Rule IDs and annotation
names are matched case-insensitively.

### `@disable [rule-id, ...]`

Suppresses diagnostics from the named rules on the line containing the
trailing comment.

```papyrus
action = 1 ; @disable float-to-int
```

Use a comma-separated list to suppress more than one rule. A bare `@disable`
with no rule IDs suppresses every lint on that line. Suppression affects
diagnostics only; it does not prevent automatic fixes from changing the line.

### `@disable-file [rule-id, ...]`

Suppresses diagnostics from the named rules throughout the file, regardless
of which line contains the annotation.

```papyrus
; @disable-file float-to-int, identifier-casing
```

A bare `@disable-file` suppresses every lint in the file. As with `@disable`,
it does not prevent automatic fixes. The `unused-disable` rule can optionally
report unknown rule IDs and annotations that suppress no findings.

The supported spelling is `@disable-file`; `@file-disable` is not recognized.
The desktop app calls the corresponding action **File disable**.

## Access annotations

Papyrus has no function or property access modifiers, so Papyrus Lint uses
trailing comment annotations to record the intended visibility in its parsed
model. Put one of these annotations on a function, event, or property header:

```papyrus
Int Property VisibleCount Auto ; @public
Int Property InternalCount Auto ; @protected

Function Reset() ; @private
EndFunction
```

### `@public`

Marks the declaration as public. This is also the default for declarations
without an access annotation.

### `@protected`

Marks the declaration as protected metadata.

### `@private`

Marks the declaration as private metadata.

These annotations describe intent to Papyrus Lint and tools that consume its
parsed syntax tree; they do not make a declaration inaccessible to Papyrus
code or add an access check to the Creation Kit compiler.
