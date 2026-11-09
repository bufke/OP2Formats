//! More of a map (tile groups, terrain types, editing and writing), drawing maps on the CPU, preparing them for a
//! GPU, and the Blight's tile masks.

use crate::*;
use op2_formats::render::{self, MapTilesets};
use op2_formats::tile_palette::{self, TilePalette};
use std::ffi::c_char;

/// Bytes of one terrain type record (`op2f_map_terrain_type`), kept raw.
pub const OP2F_TERRAIN_TYPE_BYTES: usize = 264;

/// A tile group (the editor's named blocks of tiles): its size in tiles.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fTileGroupInfo {
    pub width: u32,
    pub height: u32,
}

/// Number of tile groups.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile_group_count(map: *const Op2fMap) -> usize {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.tile_groups.len())
}

/// Tile group `index`'s size and, into `buf`, its name (some unnamed groups hold stale bytes, read lossily).
///
/// # Safety
/// `map` is a live handle; `info` writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile_group(
    map: *const Op2fMap,
    index: usize,
    info: *mut Op2fTileGroupInfo,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let group = self::index(&unsafe { obj(map, "map") }?.0.tile_groups, index, "tile group")?;
        unsafe { put_value(info, Op2fTileGroupInfo { width: group.width, height: group.height }) }?;
        unsafe { copy_str(&group.name, buf, cap, len) }
    })
}

/// Tile group `index`'s mapping indices, row by row (width * height of them), into `out`, which holds `count`.
///
/// # Safety
/// `map` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile_group_mappings(map: *const Op2fMap, index: usize, out: *mut u32, count: usize) -> Op2fStatus {
    guard(|| {
        let group = self::index(&unsafe { obj(map, "map") }?.0.tile_groups, index, "tile group")?;
        let m = &group.mapping_indices;
        if count < m.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{} mappings, room for {count}", m.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        unsafe { std::ptr::copy_nonoverlapping(m.as_ptr(), out, m.len()) };
        Ok(())
    })
}

/// Number of terrain type records.
///
/// # Safety
/// `map` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_terrain_type_count(map: *const Op2fMap) -> usize {
    unsafe { map.as_ref() }.map_or(0, |m| m.0.terrain_types.len())
}

/// Terrain type record `index` as stored (`OP2F_TERRAIN_TYPE_BYTES` bytes, not decoded) into `out`, which holds `cap`.
///
/// # Safety
/// `map` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_terrain_type(map: *const Op2fMap, index: usize, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let record = self::index(&unsafe { obj(map, "map") }?.0.terrain_types, index, "terrain type")?;
        if cap < record.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("a record has {} bytes, room for {cap}", record.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        unsafe { std::ptr::copy_nonoverlapping(record.as_ptr(), out, record.len()) };
        Ok(())
    })
}

/// Sets the tile word at (`x`, `y`) (see `Op2fTile::bits`), for editing a map before `op2f_map_write`.
///
/// # Safety
/// `map` is a live handle not used by another thread meanwhile.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_set_tile_bits(map: *mut Op2fMap, x: u32, y: u32, bits: u32) -> Op2fStatus {
    guard(|| {
        let map = &mut unsafe { map.as_mut() }.ok_or_else(|| null("map"))?.0;
        if x >= map.width || y >= map.height {
            return Err(fail(Op2fStatus::OutOfRange, format!("tile ({x}, {y}) outside the {}x{} map", map.width, map.height)));
        }
        map.set_tile_bits(x, y, bits);
        Ok(())
    })
}

/// The map as a `.map` file. An unchanged map writes back byte for byte.
///
/// # Safety
/// `map` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_write(map: *const Op2fMap, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        unsafe { put(out, Op2fBytes(map.write())) }
    })
}

/// How fast units cross a cell type, in coarse buckets (for overlays).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fSpeed {
    Fast,
    Medium,
    Slow,
    /// Vents and fumaroles.
    Special,
    Impassable,
}

/// The speed bucket of a cell type (an `Op2fCellType` value, 0-31).
#[unsafe(no_mangle)]
pub extern "C" fn op2f_cell_type_speed(cell_type: u8) -> Op2fSpeed {
    use op2_formats::map::Speed;
    match op2_formats::CellType::from_bits(cell_type & 0x1F).speed() {
        Speed::Fast => Op2fSpeed::Fast,
        Speed::Medium => Op2fSpeed::Medium,
        Speed::Slow => Op2fSpeed::Slow,
        Speed::Special => Op2fSpeed::Special,
        Speed::Impassable => Op2fSpeed::Impassable,
    }
}

/// A whole tileset strip as one image, 32 pixels wide and 32 * tile count tall.
///
/// # Safety
/// `tileset` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tileset_image(tileset: *const Op2fTileset, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let t = &unsafe { obj(tileset, "tileset") }?.0;
        let image = render::Image { width: op2_formats::TILE_SIZE, height: t.tile_count * op2_formats::TILE_SIZE, rgba: t.rgba() };
        unsafe { put(out, plain_image(image)) }
    })
}

/// Converts the game's PBMP tileset container to a standard Windows BMP file.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pbmp_to_bmp(data: *const u8, len: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let bmp = op2_formats::tileset::pbmp_to_bmp(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, Op2fBytes(bmp)) }
    })
}

// ---- Drawing maps ----

/// The tilesets a map uses, loaded from the game, for drawing it.
pub struct Op2fMapTilesets(pub(crate) MapTilesets);

/// Loads every tileset `map` names. A tileset that fails to load doesn't fail the call: its tiles draw as missing
/// (magenta and black), and `op2f_map_tilesets_status` says why.
///
/// # Safety
/// `game` and `map` are live handles; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tilesets_load(game: *const Op2fGame, map: *const Op2fMap, out: *mut *mut Op2fMapTilesets) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let map = &unsafe { obj(map, "map") }?.0;
        unsafe { put(out, Op2fMapTilesets(MapTilesets::load(&game.files, map))) }
    })
}

/// # Safety
/// `tilesets` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tilesets_free(tilesets: *mut Op2fMapTilesets) {
    unsafe { free(tilesets) }
}

/// Whether tileset slot `slot` loaded: `OP2F_STATUS_OK`, the load's error (with its message), or
/// `OP2F_STATUS_NOT_FOUND` for an empty slot.
///
/// # Safety
/// `tilesets` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tilesets_status(tilesets: *const Op2fMapTilesets, slot: usize) -> Op2fStatus {
    guard(|| {
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0.sets;
        match index(sets, slot, "tileset slot")? {
            None => Err(fail(Op2fStatus::NotFound, format!("tileset slot {slot} is empty"))),
            Some(Err(e)) => Err(fail(Op2fStatus::InvalidData, e.to_string())),
            Some(Ok(_)) => Ok(()),
        }
    })
}

/// The tile at map position (`x`, `y`) as RGBA (its mapping's first frame): `OP2F_TILE_RGBA_BYTES` into `out`, which
/// holds `cap`. `OP2F_STATUS_NOT_FOUND` when its mapping or tileset is missing.
///
/// # Safety
/// `tilesets` and `map` are live handles; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_tile_rgba(
    tilesets: *const Op2fMapTilesets,
    map: *const Op2fMap,
    x: u32,
    y: u32,
    out: *mut u8,
    cap: usize,
) -> Op2fStatus {
    guard(|| {
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0;
        let map = &unsafe { obj(map, "map") }?.0;
        if x >= map.width || y >= map.height {
            return Err(fail(Op2fStatus::OutOfRange, format!("tile ({x}, {y}) outside the {}x{} map", map.width, map.height)));
        }
        let tile = sets.tile_pixels(map, x, y).ok_or_else(|| fail(Op2fStatus::NotFound, format!("tile ({x}, {y}) has no graphic")))?;
        if cap < TILE_RGBA_BYTES {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("a tile needs {TILE_RGBA_BYTES} bytes, buffer holds {cap}")));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        let rgba = tile.rgba();
        unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), out, rgba.len()) };
        Ok(())
    })
}

/// Draws the whole map at `px` pixels per tile (1 to 32; each tile box-filtered down), as a minimap or preview.
///
/// # Safety
/// `map` and `tilesets` are live handles; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_render(
    map: *const Op2fMap,
    tilesets: *const Op2fMapTilesets,
    px: u32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0;
        unsafe { put(out, plain_image(render::compose(map, sets, px))) }
    })
}

/// Draws tile group `group` at full size; `missing` (when not NULL) receives how many of its cells had no graphic.
///
/// # Safety
/// `map` and `tilesets` are live handles; `out` writable; `missing` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_render_tile_group(
    map: *const Op2fMap,
    tilesets: *const Op2fMapTilesets,
    group: usize,
    out: *mut *mut Op2fImage,
    missing: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0;
        let g = index(&map.tile_groups, group, "tile group")?;
        let (image, absent) = render::tile_group(map, sets, g);
        if !missing.is_null() {
            unsafe { missing.write(absent) };
        }
        unsafe { put(out, plain_image(image)) }
    })
}

/// The minimap colour of mapping `mapping` (its tile's pixels averaged) as RGBA: 4 bytes into `out`.
///
/// # Safety
/// `map` and `tilesets` are live handles; `out` is 4 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_map_mapping_colour(
    map: *const Op2fMap,
    tilesets: *const Op2fMapTilesets,
    mapping: u16,
    out: *mut u8,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0;
        let colour = render::graphic_average(map, sets, mapping)
            .ok_or_else(|| fail(Op2fStatus::NotFound, format!("mapping {mapping} has no graphic")))?;
        unsafe { put_value(out.cast::<[u8; 4]>(), colour) }
    })
}

// ---- GPU preparation ----

/// Every distinct tile graphic a map uses, as layers of a texture array, so a GPU renderer needs only a layer index
/// per tile. Layer `OP2F_MISSING_LAYER` is a magenta and black checkerboard for unresolved tiles.
pub struct Op2fTilePalette(pub(crate) TilePalette);

/// The layer unresolved tiles use.
pub const OP2F_MISSING_LAYER: u32 = 0;
const _: () = assert!(OP2F_MISSING_LAYER == tile_palette::MISSING_LAYER);

/// Pixels per tile in `op2f_tile_palette_overview`.
pub const OP2F_OVERVIEW_TILE: u32 = 8;
const _: () = assert!(OP2F_OVERVIEW_TILE == tile_palette::OVERVIEW_TILE);

/// # Safety
/// `map` and `tilesets` are live handles; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_build(
    map: *const Op2fMap,
    tilesets: *const Op2fMapTilesets,
    out: *mut *mut Op2fTilePalette,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let sets = &unsafe { obj(tilesets, "tilesets") }?.0;
        unsafe { put(out, Op2fTilePalette(TilePalette::build(map, sets))) }
    })
}

/// # Safety
/// `palette` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_free(palette: *mut Op2fTilePalette) {
    unsafe { free(palette) }
}

/// Number of layers, `OP2F_TILE_RGBA_BYTES` each.
///
/// # Safety
/// `palette` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_layer_count(palette: *const Op2fTilePalette) -> u32 {
    unsafe { palette.as_ref() }.map_or(0, |p| p.0.layer_count())
}

/// The layer a tile word draws with.
///
/// # Safety
/// `palette` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_layer(palette: *const Op2fTilePalette, tile_bits: u32) -> u32 {
    unsafe { palette.as_ref() }.map_or(OP2F_MISSING_LAYER, |p| p.0.layer(op2_formats::Tile::from_bits(tile_bits)))
}

/// The RGBA pixels of `count` layers from `first`: `count * OP2F_TILE_RGBA_BYTES` bytes into `out`, which holds `cap`.
///
/// # Safety
/// `palette` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_layers(
    palette: *const Op2fTilePalette,
    first: u32,
    count: u32,
    out: *mut u8,
    cap: usize,
) -> Op2fStatus {
    guard(|| {
        let p = &unsafe { obj(palette, "palette") }?.0;
        if first.checked_add(count).is_none_or(|end| end > p.layer_count()) {
            return Err(fail(Op2fStatus::OutOfRange, format!("layers {first}..+{count} of {}", p.layer_count())));
        }
        let pixels = p.layers(first, count);
        if cap < pixels.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{} bytes, room for {cap}", pixels.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), out, pixels.len()) };
        Ok(())
    })
}

/// A low-resolution image of the whole map (`OP2F_OVERVIEW_TILE` pixels per tile), for zoomed-out views.
///
/// # Safety
/// `map` and `palette` are live handles; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tile_palette_overview(
    map: *const Op2fMap,
    palette: *const Op2fTilePalette,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let map = &unsafe { obj(map, "map") }?.0;
        let p = &unsafe { obj(palette, "palette") }?.0;
        unsafe { put(out, plain_image(tile_palette::overview(map, p))) }
    })
}

// ---- The Blight's masks ----

/// Size of `virmask.raw`.
pub const OP2F_VIRMASK_BYTES: usize = 2048;
const _: () = assert!(OP2F_VIRMASK_BYTES == op2_formats::virmask::SIZE);

/// Decodes `virmask.raw` (the Blight's 16 tile masks, its first `OP2F_VIRMASK_BYTES` bytes) into `out`: 16 masks of
/// 32 rows, each row 32 bits with the leftmost pixel in the top bit and a set bit a Blight pixel. Mask `i` is for a
/// tile whose 2x2 block has the Blight on corners `i` (1 the tile, 2 east, 4 south, 8 south-east).
///
/// # Safety
/// `data` is `len` readable bytes; `out` is 512 writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_virmask_parse(data: *const u8, len: usize, out: *mut u32) -> Op2fStatus {
    guard(|| {
        let masks = op2_formats::virmask::parse(unsafe { bytes_arg(data, len) }?)
            .ok_or_else(|| fail(Op2fStatus::InvalidData, format!("virmask.raw has {len} bytes, fewer than {OP2F_VIRMASK_BYTES}")))?;
        unsafe { put_value(out.cast::<[[u32; 32]; 16]>(), *masks) }
    })
}
