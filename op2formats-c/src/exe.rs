//! Tables read from the player's own `Outpost2.exe` at runtime.

use crate::*;
use op2_formats::exe::{self, Exe};
use std::ffi::c_char;

/// The player's `Outpost2.exe` (or another PE executable), for the tables read from it.
pub struct Op2fExe(pub(crate) Exe);

/// Number of SoundIDs (`op2f_exe_sound_files`).
pub const OP2F_SOUND_COUNT: usize = 0xFC;
const _: () = assert!(OP2F_SOUND_COUNT == exe::SOUND_COUNT);

/// Rows of the structures' ambient animation sequence (`op2f_exe_ambient_sequence`), 5 bytes each.
pub const OP2F_AMBIENT_ROWS: usize = 11;
const _: () = assert!(OP2F_AMBIENT_ROWS == exe::AMBIENT_ROWS);

/// Opens the game folder's `Outpost2.exe`.
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_open(game: *const Op2fGame, out: *mut *mut Op2fExe) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { put(out, Op2fExe(Exe::open(&game.files).map_err(lib_err)?)) }
    })
}

/// Reads a PE executable from memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_parse(data: *const u8, len: usize, out: *mut *mut Op2fExe) -> Op2fStatus {
    guard(|| {
        let exe = Exe::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, Op2fExe(exe)) }
    })
}

/// # Safety
/// `exe` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_free(exe: *mut Op2fExe) {
    unsafe { free(exe) }
}

/// The image base virtual addresses start from (0x400000 for Outpost2.exe).
///
/// # Safety
/// `exe` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_image_base(exe: *const Op2fExe) -> u32 {
    unsafe { exe.as_ref() }.map_or(0, |e| e.0.image_base())
}

/// The 32-bit value at virtual address `va`; `OP2F_STATUS_OUT_OF_RANGE` outside the file's sections.
///
/// # Safety
/// `exe` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_u32(exe: *const Op2fExe, va: u32, out: *mut u32) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        let v = exe.u32(va).ok_or_else(|| fail(Op2fStatus::OutOfRange, format!("{va:#x} isn't in the file")))?;
        unsafe { put_value(out, v) }
    })
}

/// The NUL-terminated string (up to 255 bytes) at virtual address `va`.
///
/// # Safety
/// `exe` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_cstr(exe: *const Op2fExe, va: u32, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        let s = exe.cstr(va).ok_or_else(|| fail(Op2fStatus::OutOfRange, format!("no string at {va:#x}")))?;
        unsafe { copy_str(&s, buf, cap, len) }
    })
}

/// The sound file of every SoundID: `OP2F_SOUND_COUNT` names such as "laser.wav", from Outpost2.exe 1.2's table
/// (another version gives `OP2F_STATUS_INVALID_DATA`).
///
/// # Safety
/// `exe` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_sound_files(exe: *const Op2fExe, out: *mut *mut Op2fStringList) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        unsafe { put(out, Op2fStringList(exe.sound_files().map_err(lib_err)?)) }
    })
}

/// The structures' ambient animation sequence: `OP2F_AMBIENT_ROWS` rows of 5 bytes into `out`, each a delay and then
/// the slot offset for ambient columns 1 to 4, between 0xFF-headed rows at both ends.
///
/// # Safety
/// `exe` is a live handle; `out` is `OP2F_AMBIENT_ROWS * 5` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_ambient_sequence(exe: *const Op2fExe, out: *mut u8) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        let rows = exe.ambient_sequence().map_err(lib_err)?;
        unsafe { put_value(out.cast::<[[u8; 5]; OP2F_AMBIENT_ROWS]>(), rows) }
    })
}

/// The tube and wall graphic for each set of connections: 16 bytes into `out`, indexed by a mask of the neighbours
/// (north 1, south 2, west 4, east 8), each a position in the terrain type's group of 16 graphics.
///
/// # Safety
/// `exe` is a live handle; `out` is 16 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_tube_graphics(exe: *const Op2fExe, out: *mut u8) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        unsafe { put_value(out.cast::<[u8; 16]>(), exe.tube_graphics().map_err(lib_err)?) }
    })
}

/// How long the game's Savant voice queue waits for each sound in `names` (as `op2f_exe_sound_files` gives them),
/// in ms: one value per name into `out`, which holds `count`; 0 for a file that can't be read.
///
/// # Safety
/// `game` and `names` are live handles; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sound_lengths(
    game: *const Op2fGame,
    names: *const Op2fStringList,
    out: *mut u32,
    count: usize,
) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let names = &unsafe { obj(names, "names") }?.0;
        if count < names.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{} names, room for {count}", names.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        let lengths = exe::sound_lengths(&game.files, names);
        unsafe { std::ptr::copy_nonoverlapping(lengths.as_ptr(), out, lengths.len()) };
        Ok(())
    })
}

/// How long the Savant queue waits for a sound with `data_len` bytes of samples, in ms (as for 22 kHz 8-bit mono).
#[unsafe(no_mangle)]
pub extern "C" fn op2f_savant_ms(data_len: usize) -> u32 {
    exe::savant_ms(data_len)
}

/// Whether the file is GOG's 1.2.0.7 build of Outpost2.exe, the one op2-formats is tested with.
///
/// # Safety
/// `exe` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_is_gog_build(exe: *const Op2fExe) -> bool {
    unsafe { exe.as_ref() }.is_some_and(|e| e.0.is_gog_build())
}

/// The file's SHA-1 as 40 lowercase hex digits.
///
/// # Safety
/// `exe` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_exe_sha1(exe: *const Op2fExe, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        unsafe { copy_str(&exe.sha1(), buf, cap, len) }
    })
}

// ---- The game's tables ----

/// Every table op2-formats reads from an Outpost2.exe, checked: the unit art (`op2f_vehicle_animation` and the
/// others taking this handle) and the armor values among them.
pub struct Op2fGameExe(pub(crate) std::sync::Arc<exe::GameExe>);

/// The tables of the game folder's `Outpost2.exe`. They are read once per folder and process (a few ms) and shared
/// by later calls; a failed read is tried again on the next call.
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_open(game: *const Op2fGame, out: *mut *mut Op2fGameExe) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { put(out, Op2fGameExe(exe::GameExe::of(&game.files).map_err(lib_err)?)) }
    })
}

/// The tables of an exe read with `op2f_exe_open` or `op2f_exe_parse`. A table that fails its check gives
/// `OP2F_STATUS_INVALID_DATA` naming it.
///
/// # Safety
/// `exe` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_read(exe: *const Op2fExe, out: *mut *mut Op2fGameExe) -> Op2fStatus {
    guard(|| {
        let exe = &unsafe { obj(exe, "exe") }?.0;
        unsafe { put(out, Op2fGameExe(std::sync::Arc::new(exe::GameExe::read(exe).map_err(lib_err)?))) }
    })
}

/// # Safety
/// `tables` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_free(tables: *mut Op2fGameExe) {
    unsafe { free(tables) }
}

/// Whether the tables came from GOG's 1.2.0.7 build. Another build whose tables passed their checks works, with
/// `op2f_game_exe_warning`'s caveat.
///
/// # Safety
/// `tables` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_is_gog_build(tables: *const Op2fGameExe) -> bool {
    unsafe { tables.as_ref() }.is_some_and(|t| t.0.gog_build)
}

/// What to tell the player about a build that isn't GOG's; `OP2F_STATUS_NOT_FOUND` when there is nothing to say.
///
/// # Safety
/// `tables` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_warning(tables: *const Op2fGameExe, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let t = &unsafe { obj(tables, "tables") }?.0;
        let w = t.warning().ok_or_else(|| fail(Op2fStatus::NotFound, "GOG's build: no warning"))?;
        unsafe { copy_str(&w, buf, cap, len) }
    })
}

/// Number of sheet armor classes (0 to `OP2F_ARMOR_CLASSES` - 1).
pub const OP2F_ARMOR_CLASSES: usize = 6;
const _: () = assert!(OP2F_ARMOR_CLASSES == exe::ARMOR_CLASSES);

/// The armor value the game derives from a sheet armor class (the `armor_class` of `op2f_rules_vehicle` and
/// `op2f_rules_building`), a damage factor out of 256: class 0 = 256 (none), 1 = 180, down to 5 = 40. 256 for a
/// class out of range, or a NULL handle.
///
/// # Safety
/// `tables` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_exe_armor_value(tables: *const Op2fGameExe, armor_class: u32) -> u32 {
    unsafe { tables.as_ref() }.map_or(256, |t| t.0.armor_value(armor_class))
}
