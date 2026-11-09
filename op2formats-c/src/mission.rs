//! Mission DLLs: the description the game reads from each level's DLL (type, players, tech level, the menu text, the
//! map and the tech tree), its exports and the game's file checksum, read as data without loading the DLL.

use crate::*;
use op2_formats::mission::{self, MissionDll};
use std::ffi::c_char;
use std::path::PathBuf;

/// A mission DLL's description.
pub struct Op2fMission(pub(crate) MissionDll);

/// The mission DLLs of an install.
pub struct Op2fMissionList(pub(crate) Vec<(PathBuf, Op2fMission)>);

fn mission<'a>(m: *const Op2fMission) -> Result<&'a MissionDll, Op2fStatus> {
    Ok(&unsafe { obj(m, "mission") }?.0)
}

fn list<'a>(l: *const Op2fMissionList) -> Result<&'a [(PathBuf, Op2fMission)], Op2fStatus> {
    Ok(&unsafe { obj(l, "list") }?.0)
}

/// Mission type values, as `DescBlock` stores them: a campaign mission's number is 1 to 12.
pub const OP2F_MISSION_COLONY: i32 = -1;
pub const OP2F_MISSION_AUTO_DEMO: i32 = -2;
pub const OP2F_MISSION_TUTORIAL: i32 = -3;
pub const OP2F_MISSION_LAND_RUSH: i32 = -4;
pub const OP2F_MISSION_SPACE_RACE: i32 = -5;
pub const OP2F_MISSION_RESOURCE_RACE: i32 = -6;
pub const OP2F_MISSION_MIDAS: i32 = -7;
pub const OP2F_MISSION_LAST_ONE_STANDING: i32 = -8;

/// Reads a mission DLL's description. A DLL that isn't a mission (no `DescBlock`, or without `LevelDesc`,
/// `MapName` or `TechtreeName`) is `OP2F_STATUS_INVALID_DATA`.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_open(path: *const c_char, out: *mut *mut Op2fMission) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        unsafe { put(out, Op2fMission(MissionDll::open(&path).map_err(lib_err)?)) }
    })
}

/// Reads a mission DLL's description from its bytes.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_parse(data: *const u8, len: usize, out: *mut *mut Op2fMission) -> Op2fStatus {
    guard(|| {
        let m = MissionDll::parse(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, Op2fMission(m)) }
    })
}

/// # Safety
/// `mission` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_free(mission: *mut Op2fMission) {
    unsafe { free(mission) }
}

/// `DescBlock` and `DescBlockEx`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fMissionDesc {
    /// A campaign mission's number (1 to 12), or an `OP2F_MISSION_*` value.
    pub mission_type: i32,
    /// Player slots the mission uses, AI included; for a multiplayer game the most human players.
    pub players: i32,
    /// The highest tech level the tech tree is loaded with.
    pub max_tech_level: i32,
    /// The colony report and economy update are skipped for human players.
    pub unit_only: bool,
    /// The DLL exports `DescBlockEx`.
    pub has_desc_block_ex: bool,
    /// `DescBlockEx`'s extra AI players in a multiplayer game; 0 without it.
    pub extra_ai_players: i32,
    /// The game's checksum of the whole file (see `op2f_mission_checksum`).
    pub checksum: u32,
    /// Exports, for `op2f_mission_export`.
    pub export_count: usize,
    /// Ways the file differs from what the game's own reader expects, for `op2f_mission_warning`; 0 for every
    /// shipped DLL.
    pub warning_count: usize,
}

/// # Safety
/// `mission` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_desc(mission: *const Op2fMission, out: *mut Op2fMissionDesc) -> Op2fStatus {
    guard(|| {
        let m = self::mission(mission)?;
        let desc = Op2fMissionDesc {
            mission_type: m.mission_type.value(),
            players: m.players,
            max_tech_level: m.max_tech_level,
            unit_only: m.unit_only,
            has_desc_block_ex: m.extra_ai_players.is_some(),
            extra_ai_players: m.extra_ai_players.unwrap_or(0),
            checksum: m.checksum,
            export_count: m.exports.len(),
            warning_count: m.warnings.len(),
        };
        unsafe { put_value(out, desc) }
    })
}

/// A mission's text fields.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fMissionText {
    /// `LevelDesc`: the text the game's menus show.
    LevelDesc,
    /// `MapName`: the `.map` file, a bare file name.
    MapName,
    /// `TechtreeName`: the tech tree file, a bare file name.
    TechtreeName,
}

/// Copies one of the mission's text fields (Windows-1252 bytes as Latin-1, encoded as UTF-8).
///
/// # Safety
/// `mission` is a live handle; `buf` has `cap` writable bytes (or is NULL with `cap` 0); `len` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_text(
    mission: *const Op2fMission,
    which: Op2fMissionText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let m = self::mission(mission)?;
        let s = match which {
            Op2fMissionText::LevelDesc => &m.level_desc,
            Op2fMissionText::MapName => &m.map_name,
            Op2fMissionText::TechtreeName => &m.techtree_name,
        };
        unsafe { copy_str(s, buf, cap, len) }
    })
}

/// One export.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fMissionExport {
    /// The address, relative to the image base.
    pub rva: u32,
    /// In a code section: a function rather than data.
    pub is_code: bool,
}

/// Export `index` (0 to `export_count` - 1, in the export table's name order); its name goes to `buf`.
///
/// # Safety
/// `mission` is a live handle; `out` is NULL or writable; `buf`/`len` as for `op2f_mission_text`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_export(
    mission: *const Op2fMission,
    index: usize,
    out: *mut Op2fMissionExport,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let e = crate::index(&self::mission(mission)?.exports, index, "export")?;
        if !out.is_null() {
            unsafe { out.write(Op2fMissionExport { rva: e.rva, is_code: e.is_code }) };
        }
        unsafe { copy_str(&e.name, buf, cap, len) }
    })
}

/// Whether the DLL exports `name` (case-sensitive, as the game looks names up).
///
/// # Safety
/// `mission` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_has_export(
    mission: *const Op2fMission,
    name: *const c_char,
    out: *mut bool,
) -> Op2fStatus {
    guard(|| {
        let m = self::mission(mission)?;
        let name = unsafe { str_arg(name, "name") }?;
        unsafe { put_value(out, m.has_export(name)) }
    })
}

/// Warning `index` (0 to `warning_count` - 1): a way the file differs from what the game's PE reader expects.
///
/// # Safety
/// `mission` is a live handle; `buf`/`len` as for `op2f_mission_text`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_warning(
    mission: *const Op2fMission,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let w = index_of(&self::mission(mission)?.warnings, index, "warning")?;
        unsafe { copy_str(w, buf, cap, len) }
    })
}

/// The game's checksum of a file's bytes (0x40C050): what a multiplayer game compares for the mission DLL and map.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_checksum(data: *const u8, len: usize, out: *mut u32) -> Op2fStatus {
    guard(|| {
        let data = unsafe { bytes_arg(data, len) }?;
        unsafe { put_value(out, mission::checksum(data)) }
    })
}

/// The type and player count a multiplayer mission's file name gives, the way the lobby reads it: `m`, a type letter
/// (`u`, `f`, `r`, `m`, `l`), a digit from 2 to 6. `OP2F_STATUS_NOT_FOUND` for any other name.
///
/// # Safety
/// `file_name` is a string; `mission_type` and `players` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_name_type(
    file_name: *const c_char,
    mission_type: *mut i32,
    players: *mut i32,
) -> Op2fStatus {
    guard(|| {
        let name = unsafe { str_arg(file_name, "file_name") }?;
        let (kind, n) = mission::multiplayer_name(name)
            .ok_or_else(|| fail(Op2fStatus::NotFound, format!("{name} isn't a multiplayer mission name")))?;
        unsafe { put_value(mission_type, kind.value()) }?;
        unsafe { put_value(players, n as i32) }
    })
}

// ---- An install's missions ----

/// Every mission DLL among the install's loose files (the game folder's and OPU's `base` and `maps` folders), one
/// per file name, sorted by file name; DLLs that aren't missions are left out.
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_missions(game: *const Op2fGame, out: *mut *mut Op2fMissionList) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let missions = mission::missions(&game.files).into_iter().map(|(p, m)| (p, Op2fMission(m))).collect();
        unsafe { put(out, Op2fMissionList(missions)) }
    })
}

/// # Safety
/// `list` is NULL or a live handle, not used afterwards; missions taken from it with `op2f_mission_list_get` go with
/// it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_list_free(list: *mut Op2fMissionList) {
    unsafe { free(list) }
}

/// # Safety
/// `list` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_list_count(list: *const Op2fMissionList, out: *mut usize) -> Op2fStatus {
    guard(|| unsafe { put_value(out, self::list(list)?.len()) })
}

/// Mission `index`'s full path.
///
/// # Safety
/// `list` is a live handle; `buf`/`len` as for `op2f_mission_text`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_list_path(
    list: *const Op2fMissionList,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let (path, _) = crate::index(self::list(list)?, index, "mission")?;
        unsafe { copy_str(&path.to_string_lossy(), buf, cap, len) }
    })
}

/// Mission `index`, for the `op2f_mission_*` getters. The handle belongs to the list: don't free it, and don't use it
/// after `op2f_mission_list_free`.
///
/// # Safety
/// `list` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mission_list_get(
    list: *const Op2fMissionList,
    index: usize,
    out: *mut *const Op2fMission,
) -> Op2fStatus {
    guard(|| {
        let (_, m) = crate::index(self::list(list)?, index, "mission")?;
        unsafe { put_value(out, m as *const Op2fMission) }
    })
}
