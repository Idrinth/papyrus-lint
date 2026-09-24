//! Shared project-semantic resolution contract used by cross-script lint rules.
//!
//! The lints crate deliberately has no filesystem access. Callers that can
//! resolve project scripts implement [`ExternalSignatures`]; callers checking a
//! script in isolation use [`NoExternalSignatures`].

use papyrus_parser::ast::{AccessLevel, TypeName};
use serde::Serialize;

/// A declared function parameter's name and type, as needed to resolve
/// both positional and named arguments (`func(argB = 1)`) against it.
/// Public so an [`ExternalSignatures`] implementation (e.g. the desktop
/// app's `FunctionTable`) can resolve another script's functions down to
/// full parameter info, not just types.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamInfo {
    pub name: String,
    pub type_name: TypeName,
}

/// The access level and declaring script of a resolved function or property.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberAccess {
    pub declaring_type: String,
    pub access_level: AccessLevel,
}

/// Resolves the parameters (name and type) of a function declared on some
/// other script, for callers that can look such scripts up (see the module
/// docs). Both names are matched case-insensitively; returning `None`
/// means the function couldn't be resolved and the call site is skipped.
pub trait ExternalSignatures {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>>;

    /// Resolves the access metadata of a function callable on `type_name`.
    fn function_access(&mut self, _type_name: &str, _function_name: &str) -> Option<MemberAccess> {
        None
    }

    /// Resolves the access metadata of a property available on `type_name`.
    fn property_access(&mut self, _type_name: &str, _property_name: &str) -> Option<MemberAccess> {
        None
    }

    /// Whether `sub_type` inherits from `super_type`, directly or
    /// transitively (i.e. `sub_type`'s script, or one of its ancestors'
    /// via `Extends`, is named `super_type`). Used so e.g. an `Armor`
    /// argument is accepted for a `Form` parameter.
    ///
    /// The default always says no, which keeps existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn is_subtype(&mut self, _sub_type: &str, _super_type: &str) -> bool {
        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a property named `property_name`. Both
    /// names are matched case-insensitively. Used by the "Local variable
    /// shadowing" lint (`crate::local_variable_shadowing`) to check a local
    /// variable against a parent script's properties.
    ///
    /// The default always says no, which keeps existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn has_property(&mut self, _type_name: &str, _property_name: &str) -> bool {
        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a script-level variable (a plain field,
    /// not a `Property`) named `field_name`. Both names are matched
    /// case-insensitively. Used by the "Local variable shadowing" lint
    /// (`crate::local_variable_shadowing`) to check a local variable
    /// against a parent script's fields, mirroring [`Self::has_property`]
    /// above.
    ///
    /// The default always says no, which keeps existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn has_field(&mut self, _type_name: &str, _field_name: &str) -> bool {
        false
    }

    /// Whether a script named `type_name` can be located at all — either
    /// under the project root or as a known native singleton script (e.g.
    /// `Game`, `Utility`, `Debug`). Used by the "Unresolved script
    /// reference" lint (`crate::unresolved_script`) to flag a call like
    /// `MyMissingScript.DoThing()`.
    ///
    /// The default always says yes, since a caller that can't resolve
    /// scripts (see [`NoExternalSignatures`]) has no way to tell a missing
    /// script from a legitimate one it just doesn't track — the same
    /// "unknown, don't guess" approach used by [`Self::is_subtype`] and
    /// [`Self::has_property`] above.
    fn script_exists(&mut self, _type_name: &str) -> bool {
        true
    }

    /// Whether a declared type can be resolved to a project script, a
    /// resolvable script type, or one of Papyrus's primitive types.
    /// Used by the "Unresolved script reference" lint for `Extends` and
    /// type annotations. The default remains conservative for callers
    /// without project resolution.
    fn type_exists(&mut self, _type_name: &str) -> bool {
        true
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a `State` block named `state_name`. Both
    /// names are matched case-insensitively. Used by the "GoToState state
    /// reference" lint (`crate::goto_state`) to flag a `GoToState("Name")`
    /// call whose target state can't be found anywhere in the script's own
    /// ancestry.
    ///
    /// The default always says yes, mirroring [`Self::script_exists`],
    /// since a caller that can't resolve the ancestor chain (see
    /// [`NoExternalSignatures`]) has no way to tell a missing state from one
    /// it just doesn't track.
    fn has_state(&mut self, _type_name: &str, _state_name: &str) -> bool {
        true
    }

    /// Every named `State` declared anywhere in `type_name`'s own
    /// `Extends` ancestry — starting at `type_name`'s own script, then
    /// each further ancestor it extends — as `(name, is_auto)` pairs. Used
    /// by the "Total named state count"/"Multiple Auto states" lint pair
    /// (`crate::state_count`) to tally a script's full inheritance chain
    /// against the engine's per-script limits.
    ///
    /// The default returns nothing, unlike [`Self::has_state`]'s
    /// conservative `true`: a caller that can't resolve the ancestor chain
    /// (see [`NoExternalSignatures`]) has no way to tell an over-limit
    /// ancestry from one it just doesn't track, and guessing "yes" here
    /// would mean guessing at an unbounded number of fabricated states
    /// rather than a single boolean, so it stays silent instead.
    fn ancestor_states(&mut self, _type_name: &str) -> Vec<(String, bool)> {
        Vec::new()
    }

    /// Whether `type_name`'s script declares `function_name` as a
    /// `Global` function, i.e. one callable through Papyrus's static call
    /// syntax (`ScriptName.Function(...)`) without an instance. Both names
    /// are matched case-insensitively. `None` means the function couldn't
    /// be resolved at all (unknown script or function), so the call site
    /// is left unflagged rather than guessed at — that case is instead
    /// covered by the "Unresolved script reference" lint. Used by the
    /// "Non-static function call" lint (`crate::non_global_function_call`)
    /// to flag a call like `MyScript.InstanceMethod()` whose target isn't
    /// actually declared `Global`.
    ///
    /// The default always returns `None`, keeping existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn is_global_function(&mut self, _type_name: &str, _function_name: &str) -> Option<bool> {
        None
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares `function_name` with a `; @nodiscard`
    /// directive on its header. Both names are matched case-insensitively.
    /// `None` means the function couldn't be resolved at all (unknown
    /// script or function), so the call site is left unflagged rather than
    /// guessed at. Used by the "Discarded nodiscard result" lint
    /// (`crate::unused_nodiscard`) to flag a discarded call to a function
    /// whose author marked the return value as required.
    ///
    /// The default always returns `None`, keeping existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn is_nodiscard_function(&mut self, _type_name: &str, _function_name: &str) -> Option<bool> {
        None
    }

    /// Metadata for `function_name` when `type_name` or one of its ancestors
    /// declares it as deprecated. `None` means it is not deprecated or could
    /// not be resolved.
    fn deprecated_function(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<papyrus_parser::ast::Deprecation> {
        None
    }

    /// Whether calling `function_name` on `type_name` is known to have side
    /// effects. `None` means the function could not be resolved; `false`
    /// means it was resolved but no side effect could be proven.
    ///
    /// This flag is supplied by the project-level function index so every
    /// consumer shares one definition of a side effect. The default leaves
    /// it unknown for callers that cannot resolve scripts.
    fn function_has_side_effects(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<bool> {
        None
    }

    /// Whether `type_name`'s script can be located by this resolver at
    /// all, with enough project data to answer for it. Used by the "Unused
    /// import" lint (`crate::unused_import`) as a gate before flagging an
    /// `Import` as unused: unlike [`Self::script_exists`], whose default
    /// assumes yes (so a caller that can't resolve scripts doesn't get a
    /// missing script spuriously flagged), this defaults to `false` — a
    /// caller with no project resolution (see [`NoExternalSignatures`]) has
    /// no way to tell a genuinely unused import from one it simply has no
    /// data for, so nothing is ever flagged without it.
    fn can_resolve_script(&mut self, _type_name: &str) -> bool {
        false
    }

    /// Whether `type_name`'s full `Extends` ancestry can be walked all the
    /// way to a definite root — a resolved script with no `Extends` at all —
    /// rather than trailing off at some type along the way this
    /// crate simply has no data for. Used by the "Impossible cast" lint
    /// (`crate::impossible_cast`) to tell two types *proven* unrelated
    /// (neither's fully-resolved chain reaches the other) apart from two
    /// types [`Self::is_subtype`] merely couldn't relate for lack of data —
    /// only the former is safe to flag as a cast that can never succeed.
    ///
    /// The default always says no, keeping existing behavior for callers
    /// that can't resolve scripts (see [`NoExternalSignatures`]).
    fn ancestry_fully_known(&mut self, _type_name: &str) -> bool {
        false
    }

    /// Every property type declared directly on `type_name`'s own script —
    /// not extended through `Extends`, since a property's declared type is
    /// a script-local fact and following inherited properties here would
    /// make the "Circular script dependency" lint (`crate::circular_dependency`)
    /// see phantom cycles through otherwise unrelated ancestors. Used by
    /// that lint to follow a chain of `Property` declarations across
    /// scripts, looking for one that leads back to the script it started
    /// from.
    ///
    /// The default returns nothing, mirroring [`Self::ancestor_states`]: a
    /// caller that can't resolve other scripts (see [`NoExternalSignatures`])
    /// has no way to tell an actual cycle from one it just doesn't track.
    fn property_types(&mut self, _type_name: &str) -> Vec<String> {
        Vec::new()
    }
}

/// An [`ExternalSignatures`] that never resolves anything, for checking a
/// single script in isolation.
pub struct NoExternalSignatures;

impl ExternalSignatures for NoExternalSignatures {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }
}

#[cfg(test)]
#[path = "external_signatures_tests.rs"]
mod tests;
