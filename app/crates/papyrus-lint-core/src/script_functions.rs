//! Converts a parsed Papyrus script (an AST [`Script`]) into the
//! function/property/state lookup structure [`crate::function_table::FunctionTable`]
//! caches per type name, plus the [`Member`] type it returns to editor
//! autocompletion.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use papyrus_lints::ParamInfo;
use papyrus_parser::ast::{AccessLevel, Expr, FunctionDecl, PropertyDecl, Script, Stmt, TypeName};
use papyrus_parser::token::{Token, TokenKind};

/// The parameters (name and type) and return type of a single function, as
/// declared on a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<TypeName>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
    pub access_level: AccessLevel,
    /// The name of the `State` block this signature was resolved from, or
    /// `None` when it comes from the script's empty state — either because
    /// it's declared directly on the script, or because no state overrides
    /// it (see [`ScriptFunctions::from_script`], which prefers the empty
    /// state's declaration whenever both exist, since that's the signature
    /// every ordinary call site resolves against per the language's state
    /// machine).
    pub state: Option<String>,
    /// Inner text of the `{ ... }` documentation comment on the line
    /// immediately after this function's header, if any. Placement matches
    /// `papyrus_lints::missing_doc_comment` (including backslash-continued
    /// headers). `None` when the declaration has no such comment, or the
    /// comment is empty. Carried through to editor autocompletion / hover.
    pub doc: Option<String>,
    /// Whether calling this function can change state outside its own
    /// locals: it directly assigns a property or field (its own, an
    /// inherited one, or one on another object), or it calls another
    /// function declared on the same script that does (directly or
    /// transitively, through any number of same-script calls). See
    /// [`side_effects_by_name`] for exactly what can and can't be proven
    /// this way — in particular, a call to a native engine function or to
    /// another script's function is never enough on its own to set this,
    /// since this script's own function list can't see what either one
    /// does. `false` is therefore "not provably side-effecting", not "pure".
    pub has_side_effects: bool,
    /// Whether the declaration carries a `; @nodiscard` line-comment
    /// directive (case-insensitive, word-bounded so `@nodiscardable` is
    /// not a match). Looked for on the function header's physical line(s)
    /// and on the immediately preceding source line, so both a trailing
    /// comment on the header and a dedicated comment line above it work.
    /// Tracked so later lints (and editors) can treat the function like a
    /// `Get*`-prefixed getter even when its name does not start with `Get`.
    pub nodiscard: bool,
    /// Whether the declaration carries a `; @deprecated` line-comment
    /// directive, using the same placement and word-boundary rules as
    /// `; @nodiscard`.
    pub deprecated: bool,
}

impl FunctionSignature {
    fn from_decl(
        decl: &FunctionDecl,
        doc: Option<String>,
        has_side_effects: bool,
        nodiscard: bool,
        deprecated: bool,
    ) -> Self {
        FunctionSignature {
            name: decl.name.clone(),
            params: decl
                .params
                .iter()
                .map(|p| ParamInfo {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                })
                .collect(),
            return_type: decl.return_type.clone(),
            is_global: decl.is_global,
            is_native: decl.is_native,
            is_event: decl.is_event,
            access_level: decl.access_level,
            state: decl.state.clone(),
            doc,
            has_side_effects,
            nodiscard,
            deprecated,
        }
    }
}

/// The declared type of a single property, as declared on a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropertySignature {
    pub name: String,
    pub type_name: TypeName,
    pub access_level: AccessLevel,
    /// Inner text of the `{ ... }` documentation comment on the line
    /// immediately after this property's header, if any. Same placement
    /// rules as [`FunctionSignature::doc`].
    pub doc: Option<String>,
}

impl PropertySignature {
    fn from_decl(decl: &PropertyDecl, doc: Option<String>) -> Self {
        PropertySignature {
            name: decl.name.clone(),
            type_name: decl.type_name.clone(),
            access_level: decl.access_level,
            doc,
        }
    }
}

/// A single function or property available on a script type, as returned
/// by [`crate::function_table::FunctionTable::list_members`] to drive editor
/// autocompletion.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Member {
    Function(FunctionSignature),
    Property(PropertySignature),
}

impl Member {
    /// The member's declared name, in its original case.
    pub fn name(&self) -> &str {
        match self {
            Member::Function(signature) => &signature.name,
            Member::Property(signature) => &signature.name,
        }
    }
}

/// The functions and properties declared directly on one script, plus the
/// name of the script it extends (if any), so a lookup can walk the
/// inheritance chain.
#[derive(Clone)]
pub(crate) struct ScriptFunctions {
    pub(crate) extends: Option<String>,
    pub(crate) functions: HashMap<String, FunctionSignature>,
    pub(crate) properties: HashMap<String, PropertySignature>,
    /// Every script-level variable (a plain field, not a `Property`)
    /// declared directly on this script, lowercased. Used by
    /// [`crate::function_table::FunctionTable::has_field`] so the "Local
    /// variable shadowing" lint (`papyrus_lints::local_variable_shadowing`)
    /// can also flag a local shadowing a parent script's field.
    pub(crate) variables: HashSet<String>,
    /// Each named `State` declared directly on this script, lowercased,
    /// mapped to whether it's marked `Auto`. Used by
    /// [`crate::function_table::FunctionTable::has_state`] and
    /// [`crate::function_table::FunctionTable::ancestor_states`].
    pub(crate) states: HashMap<String, bool>,
}

impl ScriptFunctions {
    pub(crate) fn from_script(script: &Script, source: &str) -> Self {
        let tokens = papyrus_parser::tokenize(source).ok();
        let doc_for = |line: usize| {
            tokens
                .as_ref()
                .and_then(|tokens| papyrus_lints::documentation_comment(source, tokens, line))
        };
        let nodiscard_for = |line: usize| nodiscard_directive(source, tokens.as_deref(), line);
        let deprecated_for = |line: usize| deprecated_directive(source, tokens.as_deref(), line);
        let mut states: HashMap<String, bool> = HashMap::new();
        for state in &script.states {
            let is_auto = states
                .entry(state.name.to_ascii_lowercase())
                .or_insert(false);
            *is_auto |= state.is_auto;
        }
        // The same first-match-wins selection `functions` below uses (empty
        // state beats a state override), but keeping the raw `FunctionDecl`
        // (bodies included) so `side_effects_by_name` has something to walk.
        let mut canonical_decls: HashMap<String, &FunctionDecl> = script
            .functions
            .iter()
            .map(|f| (f.name.to_ascii_lowercase(), f))
            .collect();
        for state in &script.states {
            for f in &state.functions {
                canonical_decls
                    .entry(f.name.to_ascii_lowercase())
                    .or_insert(f);
            }
        }
        let side_effects = side_effects_by_name(&canonical_decls);

        let mut functions: HashMap<String, FunctionSignature> = script
            .functions
            .iter()
            .map(|f| {
                let key = f.name.to_ascii_lowercase();
                let has_side_effects = side_effects.get(&key).copied().unwrap_or(false);
                (
                    key,
                    FunctionSignature::from_decl(
                        f,
                        doc_for(f.line),
                        has_side_effects,
                        nodiscard_for(f.line),
                        deprecated_for(f.line),
                    ),
                )
            })
            .collect();
        // A function declared only inside a `State` block (with no
        // matching declaration in the empty state) is still a real,
        // callable member of the script, so it belongs in the function
        // list too — callers just haven't declared its canonical empty
        // state version. A same-named empty state declaration always wins
        // over a state override here, since that's the signature every
        // ordinary (not-in-that-state) call site actually resolves
        // against.
        for state in &script.states {
            for f in &state.functions {
                let key = f.name.to_ascii_lowercase();
                functions.entry(key.clone()).or_insert_with(|| {
                    let has_side_effects = side_effects.get(&key).copied().unwrap_or(false);
                    FunctionSignature::from_decl(
                        f,
                        doc_for(f.line),
                        has_side_effects,
                        nodiscard_for(f.line),
                        deprecated_for(f.line),
                    )
                });
            }
        }
        let properties = script
            .properties
            .iter()
            .map(|p| {
                (
                    p.name.to_ascii_lowercase(),
                    PropertySignature::from_decl(p, doc_for(p.line)),
                )
            })
            .collect();
        let variables = script
            .variables
            .iter()
            .map(|v| v.name.to_ascii_lowercase())
            .collect();

        ScriptFunctions {
            extends: script.extends.clone(),
            functions,
            properties,
            variables,
            states,
        }
    }
}

/// Computes, for every function keyed (by lowercased name) in `decls`,
/// whether it has a side effect: it directly writes a property or field, or
/// it calls — directly, or transitively through any chain of same-script
/// calls — another function in `decls` that does. Starts from each
/// function's own body, then propagates through the same-script call graph
/// to a fixed point, so mutual recursion between two otherwise "pure"
/// functions is resolved correctly once either one is found to write
/// something.
///
/// A call to a native engine function or to a function on another script
/// can never turn this `true` from here, since neither one's body is
/// visible to a single script's own function list; that cross-script
/// question belongs to whatever caller can see the whole
/// `FunctionTable`, not to this per-script computation.
fn side_effects_by_name(decls: &HashMap<String, &FunctionDecl>) -> HashMap<String, bool> {
    let mut result: HashMap<String, bool> = HashMap::with_capacity(decls.len());
    let mut calls: HashMap<String, HashSet<String>> = HashMap::with_capacity(decls.len());

    for (name, decl) in decls {
        let locals = local_names(decl);
        let mut writes = false;
        let mut called = HashSet::new();
        scan_stmts(&decl.body, &locals, &mut writes, &mut called);
        result.insert(name.clone(), writes);
        calls.insert(name.clone(), called);
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (name, callees) in &calls {
            if result[name] {
                continue;
            }
            if callees
                .iter()
                .any(|callee| result.get(callee).copied().unwrap_or(false))
            {
                result.insert(name.clone(), true);
                changed = true;
            }
        }
    }

    result
}

/// Names visible as locals anywhere in `decl`'s body: its parameters, plus
/// every `VariableDecl` declared in the body. Papyrus locals aren't
/// block-scoped, so a declaration nested in an `If`/`While` still shadows a
/// same-named property/field for the rest of the function. Lowercased for
/// case-insensitive lookup.
fn local_names(decl: &FunctionDecl) -> HashSet<String> {
    let mut locals: HashSet<String> = decl
        .params
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    collect_var_decl_names(&decl.body, &mut locals);
    locals
}

fn collect_var_decl_names(body: &[Stmt], locals: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                locals.insert(decl.name.to_ascii_lowercase());
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_var_decl_names(&branch.body, locals);
                }
                collect_var_decl_names(else_body, locals);
            }
            Stmt::While { body, .. } => collect_var_decl_names(body, locals),
            _ => {}
        }
    }
}

/// Walks every statement in `body` (recursing into `If`/`While` bodies),
/// setting `*writes` once any assignment target writes a property or field
/// (see [`writes_field`]), and collecting into `called` the lowercased name
/// of every same-script function called anywhere along the way (see
/// [`scan_expr`]).
fn scan_stmts(
    body: &[Stmt],
    locals: &HashSet<String>,
    writes: &mut bool,
    called: &mut HashSet<String>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    scan_expr(value, called);
                }
            }
            Stmt::Assign { target, value, .. } => {
                if writes_field(target, locals) {
                    *writes = true;
                }
                scan_expr(target, called);
                scan_expr(value, called);
            }
            Stmt::Expr { value, .. } => scan_expr(value, called),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    scan_expr(value, called);
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    scan_expr(&branch.condition, called);
                    scan_stmts(&branch.body, locals, writes, called);
                }
                scan_stmts(else_body, locals, writes, called);
            }
            Stmt::While {
                condition, body, ..
            } => {
                scan_expr(condition, called);
                scan_stmts(body, locals, writes, called);
            }
        }
    }
}

/// Whether assigning to `target` writes a property or field rather than a
/// local: a bare name that isn't one of the function's own locals (in
/// Papyrus, that always resolves to a property or field, whether declared
/// on this script or inherited), or any `object.Property` member access —
/// writing through a member is a property write regardless of whether
/// `object` is `Self` or something else. Any other target shape (indexing
/// into an array) mutates what the reference points to, not a property or
/// field itself, so it isn't counted here.
fn writes_field(target: &Expr, locals: &HashSet<String>) -> bool {
    match target {
        Expr::Identifier(name) => !locals.contains(&name.to_ascii_lowercase()),
        Expr::Member { .. } => true,
        _ => false,
    }
}

/// Collects into `called` the lowercased name of every same-script function
/// called anywhere in `expr` (through a bare name or `Self.Name`),
/// recursing into every sub-expression so a call nested in another call's
/// arguments, a binary operand, and so on is still found. A call through
/// any other object (`akActor.Foo()`) can't be resolved without
/// cross-script information, so only its own arguments are still scanned
/// for nested same-script calls.
fn scan_expr(expr: &Expr, called: &mut HashSet<String>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            scan_expr(left, called);
            scan_expr(right, called);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, called),
        Expr::Call { callee, args, .. } => {
            if let Some(name) = called_same_script_name(callee) {
                called.insert(name);
            }
            scan_expr(callee, called);
            for arg in args {
                scan_expr(arg, called);
            }
        }
        Expr::NamedArg { value, .. } => scan_expr(value, called),
        Expr::Member { object, .. } => scan_expr(object, called),
        Expr::Index { object, index } => {
            scan_expr(object, called);
            scan_expr(index, called);
        }
        Expr::Cast { value, .. } => scan_expr(value, called),
        Expr::NewArray { size, .. } => scan_expr(size, called),
    }
}

/// The lowercased function name `callee` resolves to when it's a bare name
/// or `Self.Name` — a call this script's own function list might be able to
/// answer for. `None` for a call through any other object, which can't be
/// resolved without cross-script information.
fn called_same_script_name(callee: &Expr) -> Option<String> {
    match callee {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(property.to_ascii_lowercase())
        }
        _ => None,
    }
}

/// Whether `line` (1-indexed, the function header's starting line) is
/// marked `; @nodiscard`. Checks the immediately preceding source line and
/// every physical line of a backslash-continued header.
fn nodiscard_directive(source: &str, tokens: Option<&[Token]>, line: usize) -> bool {
    function_directive(source, tokens, line, "@nodiscard")
}

fn deprecated_directive(source: &str, tokens: Option<&[Token]>, line: usize) -> bool {
    function_directive(source, tokens, line, "@deprecated")
}

fn function_directive(
    source: &str,
    tokens: Option<&[Token]>,
    line: usize,
    directive: &str,
) -> bool {
    if line == 0 {
        return false;
    }
    let lines: Vec<&str> = source.lines().collect();
    let last = last_physical_line(line, tokens);
    let first = line.saturating_sub(1);
    // `first` is 1-indexed minus one so the line *above* the header is
    // included; clamp so we never look before the start of the file.
    let start = first.saturating_sub(1);
    lines
        .get(start..last.min(lines.len()))
        .is_some_and(|slice| slice.iter().any(|row| line_has_directive(row, directive)))
}

/// Last physical source line (1-indexed) of the logical header starting at
/// `line`. Mirrors `papyrus_lints::missing_doc_comment`'s own helper: a
/// header written on one line is just `line`; a header continued with `\`
/// resolves to the line that actually carries the terminating newline.
fn last_physical_line(line: usize, tokens: Option<&[Token]>) -> usize {
    tokens
        .and_then(|tokens| {
            tokens
                .iter()
                .find(|token| token.kind == TokenKind::Newline && token.line >= line)
                .map(|token| token.line)
        })
        .unwrap_or(line)
}

fn line_has_directive(line: &str, directive: &str) -> bool {
    let Some(comment) = line_comment_text(line) else {
        return false;
    };
    let lowered = comment.to_ascii_lowercase();
    let Some(index) = lowered.find(directive) else {
        return false;
    };
    let before_ok = index == 0
        || lowered[..index]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_whitespace() || c == ',');
    let after = &lowered[index + directive.len()..];
    let after_ok = after
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || c == ',');
    before_ok && after_ok
}

/// Text following the `;` that starts `line`'s line comment, if any.
/// Copied in spirit from `papyrus_lints::disable_comments` so function
/// tables can see the same comments the linter itself would.
fn line_comment_text(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => index += 1,
            b';' if !in_string => {
                return if bytes.get(index + 1) == Some(&b'/') {
                    None
                } else {
                    Some(&line[index + 1..])
                };
            }
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
#[path = "script_functions_tests.rs"]
mod tests;
