//! Fallback knowledge of the native (engine-defined) Papyrus class
//! hierarchy, for [`crate::function_table::FunctionTable::is_subtype`].
//!
//! Types like `Form`, `ObjectReference`, `Actor`, `MagicItem`, or `Spell`
//! are defined by the game engine itself: their `Extends` relationship
//! comes from a `.psc` source file the game ships (e.g. `Actor.psc extends
//! ObjectReference`), not from anything in the mod project being linted.
//! Most projects don't carry copies of those engine scripts under their
//! own `scripts/source`/`source/scripts`, so [`crate::script_locator`]
//! can't find them and `FunctionTable::is_subtype` has nothing to parse a
//! `Extends` chain from — meaning a perfectly legal upcast (e.g. passing an
//! `Actor` where an `ObjectReference` is expected) was flagged as a type
//! mismatch instead of silently accepted.
//!
//! This table records the immediate parent of the common native types
//! shared by Skyrim and Fallout 4, so `FunctionTable::is_subtype` can keep
//! walking a type's ancestry past the point where project resolution runs
//! out. It is deliberately not exhaustive: a type this table doesn't know
//! about (including one the linter simply has no data for, e.g. a
//! game-specific or SKSE/F4SE-added type) resolves to `None`, the same
//! "unknown, don't guess" behavior used everywhere else in this crate.
const NATIVE_EXTENDS: &[(&str, &str)] = &[
    ("objectreference", "form"),
    ("actor", "objectreference"),
    ("hazard", "objectreference"),
    ("magicitem", "form"),
    ("spell", "magicitem"),
    ("scroll", "magicitem"),
    ("ingredient", "magicitem"),
    ("potion", "magicitem"),
    ("ammo", "form"),
    ("armor", "form"),
    ("book", "form"),
    ("cell", "form"),
    ("class", "form"),
    ("combatstyle", "form"),
    ("encounterzone", "form"),
    ("equipslot", "form"),
    ("eyes", "form"),
    ("faction", "form"),
    ("flora", "form"),
    ("formlist", "form"),
    ("furniture", "form"),
    ("globalvariable", "form"),
    ("headpart", "form"),
    ("idle", "form"),
    ("imagespacemodifier", "form"),
    ("key", "form"),
    ("keyword", "form"),
    ("leveledactor", "form"),
    ("leveleditem", "form"),
    ("leveledspell", "form"),
    ("location", "form"),
    ("locationreftype", "form"),
    ("message", "form"),
    ("miscobject", "form"),
    ("musictype", "form"),
    ("outfit", "form"),
    ("package", "form"),
    ("perk", "form"),
    ("quest", "form"),
    ("race", "form"),
    ("shout", "form"),
    ("soulgem", "form"),
    ("static", "form"),
    ("textureset", "form"),
    ("topic", "form"),
    ("topicinfo", "form"),
    ("visualeffect", "form"),
    ("weapon", "form"),
    ("wordofpower", "form"),
    ("worldspace", "form"),
];

/// The immediate parent of `type_name_lower` in the native class hierarchy,
/// if this table knows it. `type_name_lower` must already be lowercased
/// (callers already work in lowercase for case-insensitive matching); the
/// returned parent name is lowercase too.
pub fn parent_of(type_name_lower: &str) -> Option<&'static str> {
    NATIVE_EXTENDS
        .iter()
        .find(|(child, _)| *child == type_name_lower)
        .map(|(_, parent)| *parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_actor_object_reference_form_chain() {
        assert_eq!(parent_of("actor"), Some("objectreference"));
        assert_eq!(parent_of("objectreference"), Some("form"));
        assert_eq!(parent_of("form"), None);
    }

    #[test]
    fn resolves_the_spell_magic_item_form_chain() {
        assert_eq!(parent_of("spell"), Some("magicitem"));
        assert_eq!(parent_of("magicitem"), Some("form"));
    }

    #[test]
    fn returns_none_for_an_unknown_type() {
        assert_eq!(parent_of("somemodsquestscript"), None);
    }
}
