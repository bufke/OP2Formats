//! Finding and checking a player's install, and the folders op2-formats keeps its own data in.

use crate::*;
use op2_formats::install;
use std::ffi::c_char;

/// Which release a game folder holds.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fRelease {
    /// GOG's release (it has GOG's `goggame-*.info`).
    Gog,
    /// The original CD release, or a copy of one.
    Cd,
}

/// What a folder holds and what it's missing.
pub struct Op2fInstallCheck(pub(crate) install::Check);

/// The installs found in the usual places.
pub struct Op2fInstallSearch(pub(crate) install::Search);

/// Checks a folder for the game's files. With `chosen`, a folder the player picked is also searched one level down
/// (and inside a Wine prefix) for the game.
///
/// # Safety
/// `dir` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check(dir: *const c_char, chosen: bool, out: *mut *mut Op2fInstallCheck) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(dir, "dir") }?;
        let check = if chosen { install::check_chosen(&dir) } else { install::check(&dir) };
        unsafe { put(out, Op2fInstallCheck(check)) }
    })
}

/// # Safety
/// `check` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_free(check: *mut Op2fInstallCheck) {
    unsafe { free(check) }
}

/// Whether the folder has every file the game needs.
///
/// # Safety
/// `check` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_ok(check: *const Op2fInstallCheck) -> bool {
    unsafe { check.as_ref() }.is_some_and(|c| c.0.ok())
}

/// # Safety
/// `check` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_release(check: *const Op2fInstallCheck, out: *mut Op2fRelease) -> Op2fStatus {
    guard(|| {
        let release = match unsafe { obj(check, "check") }?.0.release {
            install::Release::Gog => Op2fRelease::Gog,
            install::Release::Cd => Op2fRelease::Cd,
        };
        unsafe { put_value(out, release) }
    })
}

/// Whether `Outpost2.exe` is byte-identical to GOG's 1.2: 1 yes, 0 no, -1 no `Outpost2.exe`.
///
/// # Safety
/// `check` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_exe_is_gog(check: *const Op2fInstallCheck) -> i32 {
    unsafe { check.as_ref() }.and_then(|c| c.0.exe_matches_gog).map_or(-1, i32::from)
}

/// A text field of an install check.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fInstallText {
    /// The folder checked.
    Dir,
    /// The release in a few words, e.g. "GOG 1.2 + OPU 1.4.1".
    Summary,
    /// One line with details, e.g. "GOG 1.2 + OPU 1.4.1 (Outpost2.exe 1.2.0.7, GOG's)".
    Description,
    /// The problems in one short line ("missing maps.vol, e01.dll"); empty when there are none.
    ShortProblem,
    /// OPU's version ("1.4.1", or empty when unreadable); `OP2F_STATUS_NOT_FOUND` without OPU.
    OpuVersion,
    /// The OPU folder layered over the game; `OP2F_STATUS_NOT_FOUND` without one.
    OpuDir,
    /// `Outpost2.exe`'s FileVersion; `OP2F_STATUS_NOT_FOUND` when unreadable.
    ExeVersion,
}

/// # Safety
/// `check` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_text(
    check: *const Op2fInstallCheck,
    field: Op2fInstallText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let c = &unsafe { obj(check, "check") }?.0;
        let missing = |what: &str| fail(Op2fStatus::NotFound, format!("no {what}"));
        let text = match field {
            Op2fInstallText::Dir => c.dir.to_string_lossy().into_owned(),
            Op2fInstallText::Summary => c.summary(),
            Op2fInstallText::Description => c.describe(),
            Op2fInstallText::ShortProblem => c.short_problem(),
            Op2fInstallText::OpuVersion => c.opu.clone().ok_or_else(|| missing("OPU"))?,
            Op2fInstallText::OpuDir => c.opu_dir.as_ref().ok_or_else(|| missing("OPU folder"))?.to_string_lossy().into_owned(),
            Op2fInstallText::ExeVersion => c.exe_version.clone().ok_or_else(|| missing("Outpost2.exe version"))?,
        };
        unsafe { copy_str(&text, buf, cap, len) }
    })
}

/// A list field of an install check.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fInstallList {
    /// Problems worth telling the player, one per line.
    Problems,
    /// Required files not found.
    Missing,
    /// Optional files not found.
    MissingOptional,
}

/// # Safety
/// `check` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_check_list(
    check: *const Op2fInstallCheck,
    field: Op2fInstallList,
    out: *mut *mut Op2fStringList,
) -> Op2fStatus {
    guard(|| {
        let c = &unsafe { obj(check, "check") }?.0;
        let list = match field {
            Op2fInstallList::Problems => c.problems(),
            Op2fInstallList::Missing => c.missing.iter().map(|s| s.to_string()).collect(),
            Op2fInstallList::MissingOptional => c.missing_optional.iter().map(|s| s.to_string()).collect(),
        };
        unsafe { put(out, Op2fStringList(list)) }
    })
}

/// Searches the usual places (GOG, Steam, the CD's default folder, Wine and Proton prefixes, the registry) for
/// installs.
///
/// # Safety
/// `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search(out: *mut *mut Op2fInstallSearch) -> Op2fStatus {
    guard(|| unsafe { put(out, Op2fInstallSearch(install::search(&install::Env::current()))) })
}

/// # Safety
/// `search` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_free(search: *mut Op2fInstallSearch) {
    unsafe { free(search) }
}

/// Number of places searched.
///
/// # Safety
/// `search` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_searched_count(search: *const Op2fInstallSearch) -> usize {
    unsafe { search.as_ref() }.map_or(0, |s| s.0.searched.len())
}

/// Searched place `index`: its folder into `buf` and, when `origin` isn't NULL, why it was looked at into `origin`
/// (which holds `origin_cap` bytes; truncated if needed).
///
/// # Safety
/// `search` is a live handle; the buffers as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_searched(
    search: *const Op2fInstallSearch,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
    origin: *mut c_char,
    origin_cap: usize,
) -> Op2fStatus {
    guard(|| {
        let c = self::index(&unsafe { obj(search, "search") }?.0.searched, index, "place")?;
        if !origin.is_null() {
            unsafe { copy_str(&c.origin, origin, origin_cap, std::ptr::null_mut()) }?;
        }
        unsafe { copy_str(&c.dir.to_string_lossy(), buf, cap, len) }
    })
}

/// Number of game folders found, complete or not.
///
/// # Safety
/// `search` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_found_count(search: *const Op2fInstallSearch) -> usize {
    unsafe { search.as_ref() }.map_or(0, |s| s.0.found.len())
}

/// The check of found folder `index` (a new handle).
///
/// # Safety
/// `search` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_found(
    search: *const Op2fInstallSearch,
    index: usize,
    out: *mut *mut Op2fInstallCheck,
) -> Op2fStatus {
    guard(|| {
        let (_, check) = self::index(&unsafe { obj(search, "search") }?.0.found, index, "install")?;
        unsafe { put(out, Op2fInstallCheck(check.clone())) }
    })
}

/// The index of the first complete install found; `OP2F_STATUS_NOT_FOUND` when there's none.
///
/// # Safety
/// `search` is a live handle; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_install_search_best(search: *const Op2fInstallSearch, index: *mut usize) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(search, "search") }?.0;
        let best = s.found.iter().position(|(_, c)| c.ok());
        let best = best.ok_or_else(|| fail(Op2fStatus::NotFound, "no complete install found"))?;
        unsafe { put_value(index, best) }
    })
}

/// A folder op2-formats keeps its own data in.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fDataDir {
    /// The data folder itself: the platform's, or the one given to `op2f_set_data_dir`.
    Data,
    /// Where GOG's installer is unpacked (`op2f_inno_extract_game`).
    Extract,
    /// Where the OPU update is installed (`op2f_opu_install`).
    Opu,
    /// Where OPU's installer is downloaded (`op2f_opu_download`).
    Downloads,
}

/// Uses `dir` as the data folder for the rest of the process, instead of the platform's.
///
/// # Safety
/// `dir` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_set_data_dir(dir: *const c_char) -> Op2fStatus {
    guard(|| {
        install::set_data_dir(unsafe { path_arg(dir, "dir") }?);
        Ok(())
    })
}

/// # Safety
/// `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_data_dir(which: Op2fDataDir, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let env = install::Env::current();
        let dir = match which {
            Op2fDataDir::Data => env.op2_data(),
            Op2fDataDir::Extract => install::extract_dir(&env),
            Op2fDataDir::Opu => install::opu_install_dir(&env),
            Op2fDataDir::Downloads => env.op2_data().map(|d| op2_formats::opu::download_dir(&d)),
        };
        let dir = dir.ok_or_else(|| fail(Op2fStatus::NotFound, "no data folder on this system"))?;
        unsafe { copy_str(&dir.to_string_lossy(), buf, cap, len) }
    })
}

/// The OPU folder layered over a game folder: the game's own `OPU`, or the one installed in the data folder.
///
/// # Safety
/// `game_dir` is a string; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_dir(game_dir: *const c_char, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(game_dir, "game_dir") }?;
        let opu = install::opu_dir(&dir).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no OPU for {}", dir.display())))?;
        unsafe { copy_str(&opu.to_string_lossy(), buf, cap, len) }
    })
}
