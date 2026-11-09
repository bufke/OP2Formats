//! Maps and tilesets.

use crate::*;
use std::ffi::c_char;

/// A tile's movement class: the low 5 bits of its tile word ([`Op2fTile::cell_type`]). Values 12-20 are unused,
/// and 26-31 all mean a tube.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum Op2fCellType {
    FastPassible1 = 0,
    Impassible2 = 1,
    SlowPassible1 = 2,
    SlowPassible2 = 3,
    MediumPassible1 = 4,
    MediumPassible2 = 5,
    Impassible1 = 6,
    FastPassible2 = 7,
    NorthCliffs = 8,
    CliffsHighSide = 9,
    CliffsLowSide = 10,
    VentsAndFumaroles = 11,
    DozedArea = 21,
    Rubble = 22,
    NormalWall = 23,
    MicrobeWall = 24,
    LavaWall = 25,
    Tube = 26,
}

/// One map tile, decoded from its 32-bit tile word.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fTile {
    /// The packed word as stored: cell type bits 0-4, mapping index 5-15, unit index 16-26, flags 27-31.
    pub bits: u32,
    /// Index into the map's tile mappings (`op2f_map_mapping`).
    pub mapping_index: u16,
    /// An [`Op2fCellType`] value (0-31).
    pub cell_type: u8,
    pub lava: bool,
    pub lava_possible: bool,
    /// The Blight may spread from this tile.
    pub spreading: bool,
    pub microbe: bool,
    pub wall_or_building: bool,
}

/// Which tileset image and frame a tile draws.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fTileMapping {
    /// Tileset slot (`op2f_map_tileset`).
    pub tileset: u16,
    /// Tile index within that tileset.
    pub graphic: u16,
    pub animation_count: u16,
    pub animation_delay: u16,
}

/// Bytes of one 32x32 tile as RGBA.
pub const OP2F_TILE_RGBA_BYTES: usize = 4096;
const _: () = assert!(OP2F_TILE_RGBA_BYTES == TILE_RGBA_BYTES);

/// A parsed `.map` file.
pub struct Op2fMap(pub(crate) Map);

/// A decoded tileset image (a strip of 32x32 tiles).
pub struct Op2fTileset(pub(crate) Tileset);

// ---- Maps ----

/// Reads and parses a map from the game (e.g. "cm01.map").
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fMap) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let map = game.files.read_map(unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fMap(map)) }
    })
}

/// Parses a `.map` file (or saved game's map section) from memory.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_parse(data: *const u8, len: usize, out: *mut *mut Op2fMap) -> Op2fStatus {
    guard(|| {
        let map = Map::parse(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, Op2fMap(map)) }
    })
}

/// # Safety
/// `map` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_free(map: *mut Op2fMap) {
    unsafe { free(map) }
}

/// Width in tiles.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_width(map: *const Op2fMap) -> u32 {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.width)
}

/// Height in tiles.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_height(map: *const Op2fMap) -> u32 {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.height)
}

/// The file's version tag.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_version(map: *const Op2fMap) -> u32 {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.version)
}

/// Whether the map wraps around east to west (512 tiles wide).
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_wraps(map: *const Op2fMap) -> bool {
    unsafe { map.as_ref() }.is_some_and(|m| m.0.wraps())
}

/// In-game tile x minus file tile x: the game pads maps that don't wrap with 32 tiles on the west, so mission
/// coordinates are 32 more than the file's; wrapping maps have no padding. In-game y equals file y.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_x_padding(map: *const Op2fMap) -> i32 {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.x_padding())
}

/// The clip rectangle as stored: x1, y1, x2, y2.
///
/// # Safety
/// `map` is a live handle; `out` is 4 writable `int32_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_clip_rect(map: *const Op2fMap, out: *mut i32) -> Op2fStatus {
    guard(|| {
        let map = unsafe { obj(map, "map") }?;
        unsafe { put_value(out.cast::<[i32; 4]>(), map.0.clip_rect) }
    })
}

/// The tile at (`x`, `y`), in file coordinates.
///
/// # Safety
/// `map` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile(map: *const Op2fMap, x: u32, y: u32, out: *mut Op2fTile) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        if x >= map.width || y >= map.height {
            return Err(fail(Op2fStatus::OutOfRange, format!("tile ({x}, {y}) outside the {}x{} map", map.width, map.height)));
        }
        let bits = map.tile_bits(x, y);
        let t = op2_formats::Tile::from_bits(bits);
        let tile = Op2fTile {
            bits,
            mapping_index: t.mapping_index,
            cell_type: (bits & 0x1F) as u8,
            lava: t.lava,
            lava_possible: t.lava_possible,
            spreading: t.spreading,
            microbe: t.microbe,
            wall_or_building: t.wall_or_building,
        };
        unsafe { put_value(out, tile) }
    })
}

/// Copies every tile word, row by row (index `y * width + x`), into `out`, which holds `count` words;
/// `count` must be at least width * height.
///
/// # Safety
/// `map` is a live handle; `out` is `count` writable `uint32_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile_words(map: *const Op2fMap, out: *mut u32, count: usize) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let (w, h) = (map.width, map.height);
        let need = w as usize * h as usize;
        if count < need {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{w}x{h} map needs {need} words, buffer holds {count}")));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        let out = unsafe { std::slice::from_raw_parts_mut(out, need) };
        for y in 0..h {
            for x in 0..w {
                out[(y * w + x) as usize] = map.tile_bits(x, y);
            }
        }
        Ok(())
    })
}

/// Number of tileset slots; some may be empty.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tileset_count(map: *const Op2fMap) -> usize {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.tilesets.len())
}

/// Tileset slot `index`: its image's base name (e.g. "well0004"; add ".bmp" to read it) and, when `tile_count` isn't
/// NULL, its tile count. An empty slot gives an empty name and 0 tiles.
///
/// # Safety
/// `map` is a live handle; `buf` and `len` as in the string convention; `tile_count` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tileset(
    map: *const Op2fMap,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
    tile_count: *mut u32,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let slot = self::index(&map.tilesets, index, "tileset slot")?.as_ref();
        if !tile_count.is_null() {
            unsafe { tile_count.write(slot.map_or(0, |s| s.tile_count)) };
        }
        unsafe { copy_str(slot.map_or("", |s| s.name.as_str()), buf, cap, len) }
    })
}

/// Number of tile mappings.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_mapping_count(map: *const Op2fMap) -> usize {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.mappings.len())
}

/// Tile mapping `index` ([`Op2fTile::mapping_index`]).
///
/// # Safety
/// `map` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_mapping(map: *const Op2fMap, index: usize, out: *mut Op2fTileMapping) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let m = self::index(&map.mappings, index, "mapping")?;
        let mapping = Op2fTileMapping {
            tileset: m.tileset,
            graphic: m.graphic,
            animation_count: m.animation_count,
            animation_delay: m.animation_delay,
        };
        unsafe { put_value(out, mapping) }
    })
}

// ---- Tilesets ----

/// Reads and decodes a tileset from the game, by base name ("well0004") or file name ("well0004.bmp").
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fTileset) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let name = unsafe { str_arg(name, "name") }?;
        let file = if name.contains('.') { name.to_string() } else { format!("{name}.bmp") };
        let (data, source) = game.files.read(&file).map_err(lib_err)?;
        let tileset = Tileset::parse(&data).map_err(|e| lib_err(Error::Invalid(format!("decoding {source}: {e}"))))?;
        unsafe { put(out, Op2fTileset(tileset)) }
    })
}

/// Decodes a tileset (a Windows BMP or the game's PBMP) from memory.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_parse(data: *const u8, len: usize, out: *mut *mut Op2fTileset) -> Op2fStatus {
    guard(|| {
        let tileset = Tileset::parse(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, Op2fTileset(tileset)) }
    })
}

/// # Safety
/// `tileset` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_free(tileset: *mut Op2fTileset) {
    unsafe { free(tileset) }
}

/// Number of 32x32 tiles.
///
/// # Safety
/// `tileset` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_tile_count(tileset: *const Op2fTileset) -> u32 {
    unsafe { tileset.as_ref() }.map_or(0, |t| t.0.tile_count)
}

/// Tile `index` as RGBA (alpha 255), row by row: `OP2F_TILE_RGBA_BYTES` bytes into `out`, which holds `cap`.
///
/// # Safety
/// `tileset` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_tile_rgba(tileset: *const Op2fTileset, index: u32, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let tileset = &unsafe { obj(tileset, "tileset") }?.0;
        let tile = tileset.tile(index).ok_or_else(|| {
            fail(Op2fStatus::OutOfRange, format!("tile {index} out of range (count {})", tileset.tile_count))
        })?;
        if cap < TILE_RGBA_BYTES {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("a tile needs {TILE_RGBA_BYTES} bytes, buffer holds {cap}")));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        let out = unsafe { std::slice::from_raw_parts_mut(out, TILE_RGBA_BYTES) };
        let row = (op2_formats::TILE_SIZE * 4) as usize;
        for (y, dst) in out.chunks_exact_mut(row).enumerate() {
            tile.write_row(y as u32, dst);
        }
        Ok(())
    })
}

