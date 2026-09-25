//! Fallout 4's Papyrus dialect: custom `Struct`s, property `Group`s,
//! Fallout-specific declaration flags, colon-qualified names, the `is`
//! type-check operator, and remote / custom events
//! (`Event OtherScript.EventName(...)`). All are opt-in through
//! [`GameEdition::Fallout4`]. Rejection of Fallout 4 constructs in Skyrim
//! mode lives in `skyrim_mode.rs`; Starfield-only flags are rejected here
//! and accepted in `starfield_mode.rs`.
