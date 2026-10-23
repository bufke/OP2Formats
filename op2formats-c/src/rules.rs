//! The game's rules: stat sheets, tech trees and UI strings.

use crate::*;
use std::ffi::c_char;

/// A tab-separated stat sheet (`vehicles.txt`, `weapons.txt`, `building.txt`, ...).
pub struct Op2fSheet(pub(crate) Sheet);

// ---- Sheets ----

/// Reads a tab-separated sheet from the game (e.g. "vehicles.txt"); OPU's edited copies win over the stock ones,
/// as in the game.
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fSheet) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (text, _) = sheets::read_text(&game.files, unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fSheet(Sheet::parse(&text))) }
    })
}

/// Parses a sheet from memory (Windows-1252 text, as the game's are).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_parse(data: *const u8, len: usize, out: *mut *mut Op2fSheet) -> Op2fStatus {
    guard(|| {
        // Latin-1 for Windows-1252, as op2-formats reads them; the differing bytes only appear in trailing garbage.
        let text: String = unsafe { bytes_arg(data, len) }?.iter().map(|&b| b as char).collect();
        unsafe { put(out, Op2fSheet(Sheet::parse(&text))) }
    })
}

/// # Safety
/// `sheet` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_free(sheet: *mut Op2fSheet) {
    unsafe { free(sheet) }
}

/// Number of columns (from the two header rows).
///
/// # Safety
/// `sheet` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_column_count(sheet: *const Op2fSheet) -> usize {
    unsafe { sheet.as_ref() }.map_or(0, |s| s.0.columns.len())
}

/// Column `index`'s name: its two header cells joined with a space.
///
/// # Safety
/// `sheet` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_column_name(
    sheet: *const Op2fSheet,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let sheet = &unsafe { obj(sheet, "sheet") }?.0;
        unsafe { copy_str(index_of(&sheet.columns, index, "column")?, buf, cap, len) }
    })
}

/// Number of data rows.
///
/// # Safety
/// `sheet` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_row_count(sheet: *const Op2fSheet) -> usize {
    unsafe { sheet.as_ref() }.map_or(0, |s| s.0.rows.len())
}

/// Number of cells in row `row` (rows can be shorter than the header); 0 when out of range.
///
/// # Safety
/// `sheet` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_cell_count(sheet: *const Op2fSheet, row: usize) -> usize {
    unsafe { sheet.as_ref() }.and_then(|s| s.0.rows.get(row)).map_or(0, Vec::len)
}

/// The text of cell (`row`, `column`), trimmed. Column 0 is the row's code name (e.g. "Scout").
///
/// # Safety
/// `sheet` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_cell(
    sheet: *const Op2fSheet,
    row: usize,
    column: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let sheet = &unsafe { obj(sheet, "sheet") }?.0;
        let cells = index(&sheet.rows, row, "row")?;
        unsafe { copy_str(index_of(cells, column, "column")?, buf, cap, len) }
    })
}

/// Finds the row whose code name (column 0) is `code`, ignoring case.
///
/// # Safety
/// `sheet` is a live handle; `code` a string; `row` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_find_row(sheet: *const Op2fSheet, code: *const c_char, row: *mut usize) -> Op2fStatus {
    guard(|| {
        let sheet = &unsafe { obj(sheet, "sheet") }?.0;
        let code = unsafe { str_arg(code, "code") }?;
        let found = sheet.rows.iter().position(|r| r.first().is_some_and(|c| c.eq_ignore_ascii_case(code)));
        let i = found.ok_or_else(|| fail(Op2fStatus::NotFound, format!("no row {code:?} in the sheet")))?;
        unsafe { put_value(row, i) }
    })
}

