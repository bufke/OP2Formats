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
