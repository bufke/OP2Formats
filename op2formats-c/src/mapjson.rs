//! JSON maps (OP2MapJsonTools' PerRowPadded format), through op2-mapjson. A JSON map converts to and from an
//! `Op2fMap`, so everything the map functions do (drawing, tiles, tilesets) works on one.

use crate::*;
use op2_mapjson::MapJson;
use std::ffi::c_char;

/// A JSON map.
pub struct Op2fMapJson(pub(crate) MapJson);

fn mapjson_err(e: op2_mapjson::Error) -> Op2fStatus {
    let status = match &e {
        op2_mapjson::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Op2fStatus::NotFound,
        op2_mapjson::Error::Io { .. } => Op2fStatus::Io,
        _ => Op2fStatus::InvalidData,
    };
    fail(status, e.to_string())
}

fn json<'a>(j: *const Op2fMapJson) -> Result<&'a MapJson, Op2fStatus> {
    Ok(&unsafe { obj(j, "json") }?.0)
}

/// Parses JSON map text. Rows can be padded or not; the older flat arrays (OP2MapJsonTools' "Original" format)
/// aren't read. The grids must match the header's size.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_parse(data: *const u8, len: usize, out: *mut *mut Op2fMapJson) -> Op2fStatus {
    guard(|| {
        let text = std::str::from_utf8(unsafe { bytes_arg(data, len) }?).map_err(|_| fail(Op2fStatus::InvalidData, "not UTF-8 text"))?;
        unsafe { put(out, Op2fMapJson(MapJson::parse(text).map_err(mapjson_err)?)) }
    })
}

/// Reads a JSON map file.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_open(path: *const c_char, out: *mut *mut Op2fMapJson) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        unsafe { put(out, Op2fMapJson(MapJson::open(&path).map_err(mapjson_err)?)) }
    })
}

/// The JSON of a map. `map_file` (e.g. "cm01.map") goes in the header's `map` and, without its extension, `name`;
/// author and notes start empty (`op2f_mapjson_set_text`).
///
/// # Safety
/// `map` is a live handle; `map_file` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_from_map(map: *const Op2fMap, map_file: *const c_char, out: *mut *mut Op2fMapJson) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let file = unsafe { str_arg(map_file, "map_file") }?;
        unsafe { put(out, Op2fMapJson(MapJson::from_map(map, file))) }
    })
}

/// # Safety
/// `json` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_free(json: *mut Op2fMapJson) {
    unsafe { free(json) }
}

/// The JSON text, PerRowPadded as OP2MapJsonTools writes it (two-space indents, CRLF, one line per row).
///
/// # Safety
/// `json` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_write(json: *const Op2fMapJson, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| unsafe { put(out, Op2fBytes(self::json(json)?.to_json().into_bytes())) })
}

/// Writes the JSON text to a file.
///
/// # Safety
/// `json` is a live handle; `path` a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_save(json: *const Op2fMapJson, path: *const c_char) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        self::json(json)?.save(&path).map_err(mapjson_err)
    })
}

/// A JSON map's size and contents.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fMapJsonInfo {
    pub width: u32,
    pub height: u32,
    /// Tileset slots up to the last used one.
    pub source_count: usize,
    pub mapping_count: usize,
    pub terrain_type_count: usize,
    /// Whether the JSON sets a clip rect; without one, the map gets the default (x1 32, or 0 on a 512-wide map;
    /// y1 0; x2 x1 + width - 1; y2 height - 2).
    pub has_clip_rect: bool,
    /// x1, y1, x2, y2: the JSON's, or the default.
    pub clip_rect: [i32; 4],
}

/// # Safety
/// `json` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_info(json: *const Op2fMapJson, out: *mut Op2fMapJsonInfo) -> Op2fStatus {
    guard(|| {
        let j = self::json(json)?;
        let (w, h) = (j.header.width, j.header.height);
        let c = j.clip_rect.unwrap_or_else(|| op2_mapjson::default_clip_rect(w, h));
        let info = Op2fMapJsonInfo {
            width: w,
            height: h,
            source_count: j.tileset.sources.len(),
            mapping_count: j.tileset.tile_mappings.len(),
            terrain_type_count: j.tileset.terrain_types.len(),
            has_clip_rect: j.clip_rect.is_some(),
            clip_rect: [c.x1, c.y1, c.x2, c.y2],
        };
        unsafe { put_value(out, info) }
    })
}

/// A text of a JSON map's header.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fMapJsonText {
    /// The `.map` file's name (`map`).
    MapFile,
    Name,
    Author,
    Notes,
}

fn text_field(j: &mut MapJson, field: Op2fMapJsonText) -> &mut String {
    let h = &mut j.header;
    match field {
        Op2fMapJsonText::MapFile => &mut h.map,
        Op2fMapJsonText::Name => &mut h.name,
        Op2fMapJsonText::Author => &mut h.author,
        Op2fMapJsonText::Notes => &mut h.notes,
    }
}

/// # Safety
/// `json` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_text(json: *const Op2fMapJson, field: Op2fMapJsonText, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let h = &self::json(json)?.header;
        let text = match field {
            Op2fMapJsonText::MapFile => &h.map,
            Op2fMapJsonText::Name => &h.name,
            Op2fMapJsonText::Author => &h.author,
            Op2fMapJsonText::Notes => &h.notes,
        };
        unsafe { copy_str(text, buf, cap, len) }
    })
}

/// Sets a header text.
///
/// # Safety
/// `json` is a live handle not used by another thread meanwhile; `value` a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_set_text(json: *mut Op2fMapJson, field: Op2fMapJsonText, value: *const c_char) -> Op2fStatus {
    guard(|| {
        let value = unsafe { str_arg(value, "value") }?.to_string();
        let j = &mut unsafe { json.as_mut() }.ok_or_else(|| null("json"))?.0;
        *text_field(j, field) = value;
        Ok(())
    })
}

/// The map a JSON map describes. What the JSON doesn't carry (the version tag, the empty tileset slots after the
/// last used one, tile groups, and the tile words' bits above the mapping index) comes from `base` when it isn't
/// NULL (a map of the same size, e.g. the one the JSON came from), else from the shipped maps' defaults (version
/// 0x1011, 512 tileset slots, no tile groups). The width must be a power of two from 32 to 1024.
///
/// # Safety
/// `json` is a live handle; `base` NULL or a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_mapjson_to_map(json: *const Op2fMapJson, base: *const Op2fMap, out: *mut *mut Op2fMap) -> Op2fStatus {
    guard(|| {
        let j = self::json(json)?;
        let base = unsafe { base.as_ref() }.map(|b| &b.0);
        unsafe { put(out, Op2fMap(j.to_map(base).map_err(mapjson_err)?)) }
    })
}
