//! `get`/`put`/`ensure_primed` semantics for a given cache directory, built
//! on the on-disk primitives in [`crate::entry`]. [`crate::lib`] wraps these
//! with [`crate::CACHE_LOCK`] and the real `ast-cache` directory to form the
//! crate's public API.
//!
//! Split by responsibility: [`load`] is the read/lookup path, [`store`] is
//! the write/persistence path, and [`prime`] is the combined get-or-parse-
//! and-cache operation built on top of both. This file just re-exports each
//! one's entry point for [`crate::lib`] to call.

mod load;
mod prime;
mod store;

pub(crate) use load::{get_in_for_game, get_tokens_in_for_game};
pub(crate) use prime::ensure_primed_in_for_game;
pub(crate) use store::{put_in_for_game, put_tokens_in_for_game};

#[cfg(test)]
mod test_support;
