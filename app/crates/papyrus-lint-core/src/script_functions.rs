//! Converts a parsed Papyrus script (an AST [`Script`]) into the
//! function/property/state lookup structure [`crate::function_table::FunctionTable`]
//! caches per type name, plus the [`Member`] type it returns to editor
//! autocompletion.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use papyrus_lints::ParamInfo;
use papyrus_parser::ast::{
    AccessLevel, Deprecation, Expr, FunctionDecl, PropertyDecl, Script, Stmt, TypeName,
};
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
    /// Severity and migration guidance when this declaration is deprecated.
    /// `None` means it is not. Project `; @deprecated` annotations receive
    /// the generic warning used by the lint; bundled APIs retain their
    /// catalogued text.
    pub deprecation: Option<Deprecation>,
}

impl FunctionSignature {
    fn from_decl(
        decl: &FunctionDecl,
        doc: Option<String>,
        has_side_effects: bool,
        nodiscard: bool,
        deprecation: Option<Deprecation>,
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
            deprecation,
        }
    }
}
