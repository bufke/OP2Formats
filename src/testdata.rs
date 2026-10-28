//! Finding real game data from tests. The data can't be committed, so a test that needs it passes without checking
//! anything when it's missing, and says so on stderr. With `OP2_REQUIRE_DATA=1` a missing install fails the test
//! instead, so a green run proves the real-data tests actually ran. The harness's recorded runs of the original are a
//! separate requirement, `OP2_REQUIRE_RUNS=1`: only a checkout with the harness has them, and a contributor without
//! one can still require the game data. Replaying a run needs the install too, so `OP2_REQUIRE_RUNS=1` also fails a
//! missing install.

use std::path::{Path, PathBuf};

use crate::GameFiles;

/// The game folder (`default_game_dir`) when it holds an install, else `None` after reporting the skip, which fails
/// under `OP2_REQUIRE_DATA` or `OP2_REQUIRE_RUNS` (a recorded run can't be replayed without the game).
pub fn game_dir() -> Option<PathBuf> {
    let dir = crate::default_game_dir();
    // Found in any case, as GameFiles finds it (an install copied from a case-insensitive filesystem).
    if crate::find_in_dir(&dir, "Outpost2.exe").is_ok() {
        Some(dir)
    } else {
        skip_unless(&["OP2_REQUIRE_DATA", "OP2_REQUIRE_RUNS"], &format!("no Outpost 2 install at {}", dir.display()));
        None
    }
}

/// The installed game's files, else `None` as [`game_dir`] says.
pub fn game_files() -> Option<GameFiles> {
    game_dir().map(|dir| GameFiles::open(&dir).expect("opening game folder"))
}

/// The harness's recorded runs of the original game, `harness/oracle/runs` in this checkout (not committed; a
/// worktree can symlink the main checkout's). A test missing one reports it with [`skip_run`].
pub fn oracle_runs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../harness/oracle/runs")
}

/// Reports that a test is skipping its real-data checks, or panics when `OP2_REQUIRE_DATA` is set.
pub fn skip(why: &str) {
    skip_unless(&["OP2_REQUIRE_DATA"], why);
}

/// Reports that a test is skipping checks against a recorded run (or its `gamedata.json`), or panics when
/// `OP2_REQUIRE_RUNS` is set.
pub fn skip_run(why: &str) {
    skip_unless(&["OP2_REQUIRE_RUNS"], why);
}

fn skip_unless(vars: &[&str], why: &str) {
    if let Some(var) = vars.iter().find(|v| std::env::var_os(v).is_some_and(|v| !v.is_empty() && v != "0")) {
        panic!("{why} ({var} is set)");
    }
    eprintln!("skipping: {why}");
}
