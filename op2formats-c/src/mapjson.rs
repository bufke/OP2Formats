//! JSON maps (OP2MapJsonTools' PerRowPadded format), through op2-formats' `mapjson` module. A JSON map converts to
//! and from an `Op2fMap`, so everything the map functions do (drawing, tiles, tilesets) works on one.

use crate::*;
use op2_formats::mapjson::{MapJson, TileGroupJson, tilegroup};
use std::ffi::c_char;

/// A JSON map.
pub struct Op2fMapJson(pub(crate) MapJson);

fn mapjson_err(e: op2_formats::mapjson::Error) -> Op2fStatus {
    let status = match &e {
        op2_formats::mapjson::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Op2fStatus::NotFound,
        op2_formats::mapjson::Error::Io { .. } => Op2fStatus::Io,
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
        let c = j.clip_rect.unwrap_or_else(|| op2_formats::mapjson::default_clip_rect(w, h));
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

// ---- Tile group JSON ----

/// A tile group JSON file (OP2Mapper3's and OP2TileSetTools' format): one tile group, its header texts and its rows
/// of tile mapping indices.
pub struct Op2fTileGroupJson(pub(crate) TileGroupJson);

fn tilegroup<'a>(t: *const Op2fTileGroupJson) -> Result<&'a TileGroupJson, Op2fStatus> {
    Ok(&unsafe { obj(t, "tile_group") }?.0)
}

fn tilegroup_mut<'a>(t: *mut Op2fTileGroupJson) -> Result<&'a mut TileGroupJson, Op2fStatus> {
    Ok(&mut unsafe { t.as_mut() }.ok_or_else(|| null("tile_group"))?.0)
}

/// Parses tile group JSON. Rows can be padded or not, and the older flat `tiles` array (with the header's width and
/// height) reads too; when the header's size disagrees with the rows, the rows win.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_parse(data: *const u8, len: usize, out: *mut *mut Op2fTileGroupJson) -> Op2fStatus {
    guard(|| {
        let text = std::str::from_utf8(unsafe { bytes_arg(data, len) }?)
            .map_err(|e| fail(Op2fStatus::InvalidData, format!("tile group JSON isn't UTF-8: {e}")))?;
        unsafe { put(out, Op2fTileGroupJson(TileGroupJson::parse(text).map_err(mapjson_err)?)) }
    })
}

/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_open(path: *const c_char, out: *mut *mut Op2fTileGroupJson) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        unsafe { put(out, Op2fTileGroupJson(TileGroupJson::open(&path).map_err(mapjson_err)?)) }
    })
}

/// A new group of `width` by `height` tiles, all mapping index 0, with the tileset `well00` and empty texts.
///
/// # Safety
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_new(width: u32, height: u32, out: *mut *mut Op2fTileGroupJson) -> Op2fStatus {
    guard(|| {
        if width == 0 || height == 0 {
            return Err(fail(Op2fStatus::OutOfRange, format!("a {width}x{height} tile group")));
        }
        let tiles = vec![vec![0; width as usize]; height as usize];
        let g = TileGroupJson { tileset: tilegroup::DEFAULT_TILESET.into(), tiles, ..Default::default() };
        unsafe { put(out, Op2fTileGroupJson(g)) }
    })
}

/// A map's tile group `index` as JSON, with the map's tileset prefix (`well00` for the shipped maps).
///
/// # Safety
/// `map` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_from_map(map: *const Op2fMap, index: usize, out: *mut *mut Op2fTileGroupJson) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let group = crate::index(&map.tile_groups, index, "tile group")?;
        let tileset = tilegroup::tileset_prefix(map).unwrap_or_else(|| tilegroup::DEFAULT_TILESET.into());
        unsafe { put(out, Op2fTileGroupJson(TileGroupJson::from_map_group(group, &tileset).map_err(mapjson_err)?)) }
    })
}

/// # Safety
/// `tile_group` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_free(tile_group: *mut Op2fTileGroupJson) {
    unsafe { free(tile_group) }
}

/// The JSON text as OP2Mapper3 and OP2TileSetTools write it (two-space indents, CRLF, one line per row, numbers
/// padded to 4).
///
/// # Safety
/// `tile_group` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_write(tile_group: *const Op2fTileGroupJson, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| unsafe { put(out, Op2fBytes(tilegroup(tile_group)?.to_json().into_bytes())) })
}

/// # Safety
/// `tile_group` is a live handle; `path` a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_save(tile_group: *const Op2fTileGroupJson, path: *const c_char) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        tilegroup(tile_group)?.save(&path).map_err(mapjson_err)
    })
}

/// The group's size in tiles (from its rows).
///
/// # Safety
/// `tile_group` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_info(tile_group: *const Op2fTileGroupJson, out: *mut Op2fTileGroupInfo) -> Op2fStatus {
    guard(|| {
        let g = tilegroup(tile_group)?;
        unsafe { put_value(out, Op2fTileGroupInfo { width: g.width() as u32, height: g.height() as u32 }) }
    })
}

/// The group's mapping indices, row by row (width * height of them), into `out`, which holds `count`.
///
/// # Safety
/// `tile_group` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_tiles(tile_group: *const Op2fTileGroupJson, out: *mut u16, count: usize) -> Op2fStatus {
    guard(|| {
        let g = tilegroup(tile_group)?;
        let n = g.width() * g.height();
        if count < n {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{n} tiles, room for {count}")));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        for (i, &v) in g.tiles.iter().flatten().enumerate() {
            unsafe { out.add(i).write(v) };
        }
        Ok(())
    })
}

/// Replaces the group's tiles: `width` * `height` mapping indices (0 to 2047), row by row.
///
/// # Safety
/// `tile_group` is a live handle not used by another thread meanwhile; `tiles` is `width` * `height` readable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_set_tiles(
    tile_group: *mut Op2fTileGroupJson,
    width: u32,
    height: u32,
    tiles: *const u16,
) -> Op2fStatus {
    guard(|| {
        let (w, h) = (width as usize, height as usize);
        if w == 0 || h == 0 {
            return Err(fail(Op2fStatus::OutOfRange, format!("a {width}x{height} tile group")));
        }
        if tiles.is_null() {
            return Err(null("tiles"));
        }
        let values = unsafe { std::slice::from_raw_parts(tiles, w * h) };
        if let Some(v) = values.iter().find(|&&v| v > tilegroup::MAX_MAPPING_INDEX) {
            return Err(fail(Op2fStatus::OutOfRange, format!("tile {v} isn't a mapping index (0 to 2047)")));
        }
        tilegroup_mut(tile_group)?.tiles = values.chunks(w).map(<[u16]>::to_vec).collect();
        Ok(())
    })
}

/// A tile group's header texts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fTileGroupText {
    /// The tileset's 6-character prefix, `well00` for the game's.
    Tileset,
    Name,
    /// The picture's file name, `<name>.bmp`, or empty.
    Bmp,
    Notes,
}

fn tilegroup_text(g: &TileGroupJson, field: Op2fTileGroupText) -> &String {
    match field {
        Op2fTileGroupText::Tileset => &g.tileset,
        Op2fTileGroupText::Name => &g.name,
        Op2fTileGroupText::Bmp => &g.bmp,
        Op2fTileGroupText::Notes => &g.notes,
    }
}

/// # Safety
/// `tile_group` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_text(
    tile_group: *const Op2fTileGroupJson,
    field: Op2fTileGroupText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| unsafe { copy_str(tilegroup_text(tilegroup(tile_group)?, field), buf, cap, len) })
}

/// # Safety
/// `tile_group` is a live handle not used by another thread meanwhile; `value` a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tilegroup_set_text(
    tile_group: *mut Op2fTileGroupJson,
    field: Op2fTileGroupText,
    value: *const c_char,
) -> Op2fStatus {
    guard(|| {
        let value = unsafe { str_arg(value, "value") }?.to_string();
        let g = tilegroup_mut(tile_group)?;
        *match field {
            Op2fTileGroupText::Tileset => &mut g.tileset,
            Op2fTileGroupText::Name => &mut g.name,
            Op2fTileGroupText::Bmp => &mut g.bmp,
            Op2fTileGroupText::Notes => &mut g.notes,
        } = value;
        Ok(())
    })
}

/// Adds the group to a map's tile groups, at the end (its name and tiles; the other texts have no place in a
/// `.map`). The map's mappings aren't checked: the indices are placed as they are.
///
/// # Safety
/// `map` is a live handle not used by another thread meanwhile; `tile_group` a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_add_tile_group(map: *mut Op2fMap, tile_group: *const Op2fTileGroupJson) -> Op2fStatus {
    guard(|| {
        let group = tilegroup(tile_group)?.to_map_group();
        unsafe { map.as_mut() }.ok_or_else(|| null("map"))?.0.tile_groups.push(group);
        Ok(())
    })
}

/// Removes a map's tile group `index`; the later ones move down.
///
/// # Safety
/// `map` is a live handle not used by another thread meanwhile.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_remove_tile_group(map: *mut Op2fMap, index: usize) -> Op2fStatus {
    guard(|| {
        let groups = &mut unsafe { map.as_mut() }.ok_or_else(|| null("map"))?.0.tile_groups;
        crate::index(groups, index, "tile group")?;
        groups.remove(index);
        Ok(())
    })
}
