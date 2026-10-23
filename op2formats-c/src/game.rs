//! The game folder: opening one, reading its files, and finding and checking installs.

use crate::*;
use std::ffi::c_char;
use std::path::PathBuf;

/// An opened game folder: loose files (with OPU's overrides) and `.vol` archives, found by name.
pub struct Op2fGame {
    pub(crate) files: GameFiles,
    pub(crate) map_names: Vec<String>,
}

// ---- Game folder ----

/// Opens an Outpost 2 folder. With `dir` NULL: `$OP2_DIR`, else the first complete install found in the usual
/// places (GOG, Steam, registry). Files are looked up the way OPU layers them: `OPU/base`, `OPU/maps`, the folder
/// itself, then its `.vol` archives; names match case-insensitively.
///
/// # Safety
/// `dir` is NULL or a string; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_open(dir: *const c_char, out: *mut *mut Op2fGame) -> Op2fStatus {
    guard(|| {
        let dir = if dir.is_null() { op2_formats::default_game_dir() } else { PathBuf::from(unsafe { str_arg(dir, "dir") }?) };
        if !dir.is_dir() {
            return Err(fail(Op2fStatus::NotFound, format!("game folder {} doesn't exist", dir.display())));
        }
        let files = GameFiles::open(&dir).map_err(lib_err)?;
        let map_names = files.map_names();
        unsafe { put(out, Op2fGame { files, map_names }) }
    })
}

/// # Safety
/// `game` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_free(game: *mut Op2fGame) {
    unsafe { free(game) }
}

/// The folder the game was opened from.
///
/// # Safety
/// `game` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_dir(game: *const Op2fGame, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { copy_str(&game.files.root().to_string_lossy(), buf, cap, len) }
    })
}

/// Reads a game file by name (e.g. "cm01.map", "vehicles.txt", "well0004.bmp"), decompressing `.vol` entries.
/// A name with a folder in it is read as a path.
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_read(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (data, _) = game.files.read(unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fBytes(data)) }
    })
}

/// Where `name` would be read from: a file path, or "archive.vol:name".
///
/// # Safety
/// `game` is a live handle; `name` a string; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_source(
    game: *const Op2fGame,
    name: *const c_char,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (_, source) = game.files.read(unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { copy_str(&source.to_string(), buf, cap, len) }
    })
}

/// Number of distinct `.map` files in the game.
///
/// # Safety
/// `game` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_map_count(game: *const Op2fGame) -> usize {
    unsafe { game.as_ref() }.map_or(0, |g| g.map_names.len())
}

/// The name of map `index` (0 to `op2f_game_map_count` - 1), sorted case-insensitively.
///
/// # Safety
/// `game` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_map_name(
    game: *const Op2fGame,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { copy_str(index_of(&game.map_names, index, "map")?, buf, cap, len) }
    })
}

/// The loose file `name` resolves to (OPU's overrides first), for files too big to read whole, like `op2.clm`;
/// `OP2F_STATUS_NOT_FOUND` when it is only in a `.vol` archive, or nowhere.
///
/// # Safety
/// `game` is a live handle; `name` a string; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_loose_path(
    game: *const Op2fGame,
    name: *const c_char,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let name = unsafe { str_arg(name, "name") }?;
        let path = game.files.loose_path(name).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no loose file {name}")))?;
        unsafe { copy_str(&path.to_string_lossy(), buf, cap, len) }
    })
}

/// The `.vol` archives in the game folder, in the order they are searched (open one with `op2f_vol_open`).
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_vols(game: *const Op2fGame, out: *mut *mut Op2fStringList) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { put(out, Op2fStringList(game.files.vols().map(|(name, _)| name.to_string()).collect())) }
    })
}

/// The file `name` in `dir` itself (no OPU overrides), matched case-insensitively as Windows would.
///
/// # Safety
/// `dir` and `name` are strings; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_find_in_dir(
    dir: *const c_char,
    name: *const c_char,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(dir, "dir") }?;
        let name = unsafe { str_arg(name, "name") }?;
        let path = op2_formats::find_in_dir(&dir, name).map_err(|e| match e {
            op2_formats::Error::Io { .. } => lib_err(e),
            e => fail(Op2fStatus::NotFound, e.to_string()),
        })?;
        unsafe { copy_str(&path.to_string_lossy(), buf, cap, len) }
    })
}

/// Sets the game folder `op2f_game_open(NULL, ...)` and `op2f_default_game_dir` use for the rest of the process.
///
/// # Safety
/// `dir` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_set_game_dir(dir: *const c_char) -> Op2fStatus {
    guard(|| {
        op2_formats::set_game_dir(unsafe { path_arg(dir, "dir") }?);
        Ok(())
    })
}

/// The game folder `op2f_game_open(NULL, ...)` opens: the one given to `op2f_set_game_dir`, else `$OP2_DIR`, else
/// the first complete install found in the usual places; with none, a path that doesn't exist.
///
/// # Safety
/// `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_default_game_dir(buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| unsafe { copy_str(&op2_formats::default_game_dir().to_string_lossy(), buf, cap, len) })
}

