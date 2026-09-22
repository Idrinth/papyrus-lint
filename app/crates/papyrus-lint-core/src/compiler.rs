//! Runs `PapyrusCompiler.exe` against a single `.psc` script, reproducing
//! the invocation Creation Kit tooling uses to compile one script out of
//! its source directory:
//!
//! ```text
//! PapyrusCompiler.exe "<script path>" -i="<source dir 1>;<source dir 2>" -o="<output dir>" -f="TESV_Papyrus_Flags.flg"
//! ```
//!
//! `<script path>` names the `.psc` file being compiled. Its parent is
//! conventionally `scripts/source` or `source/scripts` under a project's root — see
//! [`crate::script_locator`]) and `<output dir>` is its parent, matching
//! the layout Bethesda's tooling expects: a `Source` directory holding
//! `.psc` files sits inside the `Scripts` directory that receives the
//! compiled `.pex` output.
//!
//! `-i` accepts multiple import directories separated by `;`, so it's
//! always given both of [`crate::script_locator`]'s known source
//! directories under the project root, not just the one the script being
//! compiled happens to live in — letting it import from either layout —
//! plus any of the project's configured `additional_script_roots` (see
//! [`papyrus_lint_config::load_script_roots`]), so a script that
//! imports from a shared library location outside those two conventional
//! directories still compiles. The compiler is run with its own containing
//! directory as the working directory, so it can resolve the target game's
//! bundled flags file named by the trailing `-f` argument via its relative
//! path.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::pex_header;

/// Returns the compiler flags file shipped for `game`.
///
/// Keeping this selection exhaustive means adding another [`papyrus_lints::Game`]
/// cannot silently reuse Skyrim's flags.
fn flags_file(game: papyrus_lints::Game) -> &'static str {
    match game {
        papyrus_lints::Game::Skyrim => "TESV_Papyrus_Flags.flg",
        papyrus_lints::Game::Fallout4 => "Institute_Papyrus_Flags.flg",
    }
}
