//! Converts a parsed Papyrus script (an AST [`Script`]) into the
//! function/property/state lookup structure [`crate::function_table::FunctionTable`]
//! caches per type name, plus the [`Member`] type it returns to editor
//! autocompletion.

use std::collections::HashMap;

use serde::Serialize;

use papyrus_lints::ParamInfo;
use papyrus_parser::ast::{FunctionDecl, PropertyDecl, Script, TypeName};

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
}

impl FunctionSignature {
    fn from_decl(decl: &FunctionDecl, doc: Option<String>) -> Self {
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
            state: decl.state.clone(),
            doc,
        }
    }
}

/// The declared type of a single property, as declared on a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropertySignature {
    pub name: String,
    pub type_name: TypeName,
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
        let mut states: HashMap<String, bool> = HashMap::new();
        for state in &script.states {
            let is_auto = states
                .entry(state.name.to_ascii_lowercase())
                .or_insert(false);
            *is_auto |= state.is_auto;
        }
        let mut functions: HashMap<String, FunctionSignature> = script
            .functions
            .iter()
            .map(|f| {
                (
                    f.name.to_ascii_lowercase(),
                    FunctionSignature::from_decl(f, doc_for(f.line)),
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
                functions
                    .entry(f.name.to_ascii_lowercase())
                    .or_insert_with(|| FunctionSignature::from_decl(f, doc_for(f.line)));
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

        ScriptFunctions {
            extends: script.extends.clone(),
            functions,
            properties,
            states,
        }
    }
}
