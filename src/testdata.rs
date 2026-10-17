//! Finding real game data from tests. The data can't be committed, so a test that needs it passes without checking
//! anything when it's missing, and says so on stderr. With `OP2_REQUIRE_DATA=1` a missing install or missing oracle
//! runs fail the test instead, so a green run proves the real-data tests actually ran.

use std::path::{Path, PathBuf};

use crate::GameFiles;

/// The game folder (`default_game_dir`) when it holds an install, else `None` after [`skip`].
pub fn game_dir() -> Option<PathBuf> {
    let dir = crate::default_game_dir();
    if dir.join("Outpost2.exe").exists() {
        Some(dir)
    } else {
        skip(&format!("no Outpost 2 install at {}", dir.display()));
        None
    }
}

/// The installed game's files, else `None` after [`skip`].
pub fn game_files() -> Option<GameFiles> {
    game_dir().map(|dir| GameFiles::open(&dir).expect("opening game folder"))
}

/// The harness's recorded runs of the original game, `harness/oracle/runs` in this checkout (not committed; a
/// worktree can symlink the main checkout's).
pub fn oracle_runs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harness/oracle/runs")
}

/// Reports that a test is skipping its real-data checks, or panics when `OP2_REQUIRE_DATA` is set.
pub fn skip(why: &str) {
    if std::env::var_os("OP2_REQUIRE_DATA").is_some_and(|v| !v.is_empty() && v != "0") {
        panic!("{why} (OP2_REQUIRE_DATA is set)");
    }
    eprintln!("skipping: {why}");
}
