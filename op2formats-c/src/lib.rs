//! C interface to op2-formats, for C, C++ and any language that can call a C library.
//!
//! `include/op2formats.h` is generated from this file by cbindgen; the calling conventions (handles, status codes,
//! string buffers, panics) are written at its top, in `cbindgen.toml`. Keep them true when changing this file.

use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use op2_formats::sheets::{self, Sheet};
use op2_formats::units;
use op2_formats::{Error, GameFiles, Map, Sprite, Sprites, TILE_RGBA_BYTES, Tileset, Tint, VolArchive};

/// Result of a call that can fail.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fStatus {
    Ok = 0,
    /// A required pointer argument was NULL.
    NullArgument,
    /// A string argument isn't valid UTF-8.
    InvalidUtf8,
    /// An index or coordinate is out of range.
    OutOfRange,
    /// The output buffer was too small: it holds a truncated copy, and `*len` the full length.
    BufferTooSmall,
    /// The file or folder doesn't exist.
    NotFound,
    /// Reading a file failed.
    Io,
    /// The data is malformed, or not the expected format.
    InvalidData,
    /// The data uses something the library doesn't read (e.g. an unknown compression).
    Unsupported,
    /// A bug in the library; the message says where.
    Panic,
}

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

/// An opened game folder: loose files (with OPU's overrides) and `.vol` archives, found by name.
pub struct Op2fGame {
    files: GameFiles,
    map_names: Vec<String>,
}

/// A block of bytes the library allocated.
pub struct Op2fBytes(Vec<u8>);

/// A parsed `.map` file.
pub struct Op2fMap(Map);

/// A decoded tileset image (a strip of 32x32 tiles).
pub struct Op2fTileset(Tileset);

/// A tab-separated stat sheet (`vehicles.txt`, `weapons.txt`, `building.txt`, ...).
pub struct Op2fSheet(Sheet);

/// A `.vol` archive.
pub struct Op2fVol(VolArchive);

/// The game's sprites: `op2_art.prt` (palettes, images, animations) with the pixels in `OP2_ART.BMP` and the
/// player colours in `color.bmp`.
pub struct Op2fSprites(Sprites);

/// An RGBA image the library drew: a sprite image or a composited animation frame.
pub struct Op2fImage(Sprite);

/// Number of player colours (`color.bmp` ramps): blue, red, green, yellow, cyan, magenta.
pub const OP2F_PLAYER_COLORS: i32 = 6;
const _: () = assert!(OP2F_PLAYER_COLORS as usize == op2_formats::art::PLAYER_COLORS);

/// One image in `op2_art.prt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fImageInfo {
    pub width: u32,
    pub height: u32,
    /// The image's type bits as stored. Bit 0: an in-game graphic (else a menu one); bit 2: a 1-bit shadow.
    pub kind: u16,
    /// Index of its palette.
    pub palette: u16,
    /// A 1-bit shadow, drawn as translucent black.
    pub shadow: bool,
}

/// One animation in `op2_art.prt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fAnimationInfo {
    pub frame_count: u32,
    /// The frame pixel that sits on the object's position (what `op2f_image_hotspot_x`/`_y` give for its frames).
    pub displacement_x: i32,
    pub displacement_y: i32,
    /// Selection box, in the same frame coordinates: left, top, right, bottom.
    pub selection: [i32; 4],
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Records `message` as this thread's last error and returns `status`.
fn fail(status: Op2fStatus, message: impl Into<String>) -> Op2fStatus {
    LAST_ERROR.with(|e| *e.borrow_mut() = message.into());
    status
}

fn lib_err(e: Error) -> Op2fStatus {
    let status = match e {
        Error::NotFound(_) => Op2fStatus::NotFound,
        Error::Io { .. } => Op2fStatus::Io,
        Error::Unsupported(_) => Op2fStatus::Unsupported,
        Error::Truncated { .. } | Error::BadTag { .. } | Error::Invalid(_) => Op2fStatus::InvalidData,
    };
    fail(status, e.to_string())
}

/// Runs `f`, turning a panic into [`Op2fStatus::Panic`].
fn guard(f: impl FnOnce() -> Result<(), Op2fStatus>) -> Op2fStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => Op2fStatus::Ok,
        Ok(Err(status)) => status,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            fail(Op2fStatus::Panic, format!("internal error in op2formats: {msg}"))
        }
    }
}

fn null(what: &str) -> Op2fStatus {
    fail(Op2fStatus::NullArgument, format!("{what} is NULL"))
}

/// # Safety
/// `p` is NULL or a NUL-terminated string that stays valid for `'a`.
unsafe fn str_arg<'a>(p: *const c_char, what: &str) -> Result<&'a str, Op2fStatus> {
    if p.is_null() {
        return Err(null(what));
    }
    unsafe { CStr::from_ptr(p) }.to_str().map_err(|_| fail(Op2fStatus::InvalidUtf8, format!("{what} isn't valid UTF-8")))
}

/// # Safety
/// `p` is NULL or a live handle that stays valid for `'a`.
unsafe fn obj<'a, T>(p: *const T, what: &str) -> Result<&'a T, Op2fStatus> {
    unsafe { p.as_ref() }.ok_or_else(|| null(what))
}

/// Moves `value` into a new handle written to `*out`.
///
/// # Safety
/// `out` is NULL or writable.
unsafe fn put<T>(out: *mut *mut T, value: T) -> Result<(), Op2fStatus> {
    if out.is_null() {
        return Err(null("out"));
    }
    unsafe { out.write(Box::into_raw(Box::new(value))) };
    Ok(())
}

/// # Safety
/// `out` is NULL or writable.
unsafe fn put_value<T>(out: *mut T, value: T) -> Result<(), Op2fStatus> {
    if out.is_null() {
        return Err(null("out"));
    }
    unsafe { out.write(value) };
    Ok(())
}

/// Copies `s` into the caller's buffer as described in the module docs.
///
/// # Safety
/// `buf` is NULL or `cap` writable bytes; `len` is NULL or writable.
unsafe fn copy_str(s: &str, buf: *mut c_char, cap: usize, len: *mut usize) -> Result<(), Op2fStatus> {
    if !len.is_null() {
        unsafe { len.write(s.len()) };
    }
    if buf.is_null() {
        return if cap == 0 { Ok(()) } else { Err(null("buf")) };
    }
    if cap == 0 {
        return Err(fail(Op2fStatus::BufferTooSmall, "buffer has no room"));
    }
    let n = s.len().min(cap - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf.cast::<u8>(), n);
        buf.add(n).write(0);
    }
    if n < s.len() {
        return Err(fail(Op2fStatus::BufferTooSmall, format!("string needs {} bytes plus the NUL, buffer holds {cap}", s.len())));
    }
    Ok(())
}

/// # Safety
/// `data` is NULL or `len` readable bytes.
unsafe fn bytes_arg<'a>(data: *const u8, len: usize) -> Result<&'a [u8], Op2fStatus> {
    match (data.is_null(), len) {
        (true, 0) => Ok(&[]),
        (true, _) => Err(null("data")),
        (false, _) => Ok(unsafe { std::slice::from_raw_parts(data, len) }),
    }
}

fn index<'a, T>(items: &'a [T], i: usize, what: &str) -> Result<&'a T, Op2fStatus> {
    items.get(i).ok_or_else(|| fail(Op2fStatus::OutOfRange, format!("{what} {i} out of range (count {})", items.len())))
}

/// # Safety
/// `p` is NULL or a pointer from `Box::into_raw`, not used again.
unsafe fn free<T>(p: *mut T) {
    if !p.is_null() {
        // Dropping parsed data can't panic, so no guard.
        drop(unsafe { Box::from_raw(p) });
    }
}

// ---- Library ----

/// The library's version, a static NUL-terminated string such as "0.1.0".
#[unsafe(no_mangle)]
pub extern "C" fn op2f_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Copies the message of this thread's most recent failure (empty if none) into `buf`; returns its full length.
///
/// # Safety
/// `buf` is NULL or `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_last_error(buf: *mut c_char, cap: usize) -> usize {
    let msg = LAST_ERROR.with(|e| e.borrow().clone());
    // A failure here would overwrite the message being read; the length is what's returned anyway.
    let _ = unsafe { copy_str(&msg, buf, cap, std::ptr::null_mut()) };
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.clone());
    msg.len()
}

/// Frees a byte block. NULL does nothing.
///
/// # Safety
/// `bytes` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_free(bytes: *mut Op2fBytes) {
    unsafe { free(bytes) }
}

/// The bytes of a block (valid until it is freed).
///
/// # Safety
/// `bytes` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_data(bytes: *const Op2fBytes) -> *const u8 {
    unsafe { bytes.as_ref() }.map_or(std::ptr::null(), |b| b.0.as_ptr())
}

/// # Safety
/// `bytes` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_len(bytes: *const Op2fBytes) -> usize {
    unsafe { bytes.as_ref() }.map_or(0, |b| b.0.len())
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

fn index_of<'a>(names: &'a [String], i: usize, what: &str) -> Result<&'a str, Op2fStatus> {
    index(names, i, what).map(String::as_str)
}

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

// ---- Sprites ----

/// Loads the game's sprites (`op2_art.prt`, `OP2_ART.BMP` and, when present, `color.bmp`).
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_open(game: *const Op2fGame, out: *mut *mut Op2fSprites) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let sprites = Sprites::load(&game.files).map_err(lib_err)?;
        unsafe { put(out, Op2fSprites(sprites)) }
    })
}

/// # Safety
/// `sprites` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_free(sprites: *mut Op2fSprites) {
    unsafe { free(sprites) }
}

/// Number of images.
///
/// # Safety
/// `sprites` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_count(sprites: *const Op2fSprites) -> usize {
    unsafe { sprites.as_ref() }.map_or(0, |s| s.0.art.images.len())
}

/// Number of animations.
///
/// # Safety
/// `sprites` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_count(sprites: *const Op2fSprites) -> usize {
    unsafe { sprites.as_ref() }.map_or(0, |s| s.0.art.animations.len())
}

/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_info(sprites: *const Op2fSprites, index: usize, out: *mut Op2fImageInfo) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let meta = self::index(&sprites.art.images, index, "image")?;
        let info = Op2fImageInfo {
            width: meta.width,
            height: meta.height,
            kind: meta.kind,
            palette: meta.palette,
            shadow: meta.is_shadow(),
        };
        unsafe { put_value(out, info) }
    })
}

/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_info(
    sprites: *const Op2fSprites,
    animation: usize,
    out: *mut Op2fAnimationInfo,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let a = index(&sprites.art.animations, animation, "animation")?;
        let r = a.selection;
        let info = Op2fAnimationInfo {
            frame_count: a.frames.len() as u32,
            displacement_x: a.displacement.0,
            displacement_y: a.displacement.1,
            selection: [r.left, r.top, r.right, r.bottom],
        };
        unsafe { put_value(out, info) }
    })
}

/// The colours to draw with: `player` < 0 for the palette as stored (player-coloured parts in player 0's blue),
/// else that player's colour (0 to `OP2F_PLAYER_COLORS` - 1).
fn tint(player: i32) -> Result<Tint, Op2fStatus> {
    match player {
        ..0 => Ok(Tint::Stored),
        0..OP2F_PLAYER_COLORS => Ok(Tint::Player(player as u8)),
        _ => Err(fail(Op2fStatus::OutOfRange, format!("player colour {player} out of range (0 to {})", OP2F_PLAYER_COLORS - 1))),
    }
}

/// Draws image `index` on its own as RGBA (index 0 transparent, shadows translucent black); its hotspot is (0, 0).
/// `player` as in `op2f_sprites_frame`.
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image(
    sprites: *const Op2fSprites,
    index: usize,
    player: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        self::index(&sprites.art.images, index, "image")?;
        let image = sprites.image_rgba(index, tint(player)?).map_err(lib_err)?;
        unsafe { put(out, Op2fImage(Sprite { image, hotspot: (0, 0) })) }
    })
}

/// Composites frame `frame` of animation `animation` (its layers drawn in order) as RGBA. `player` < 0 draws the
/// palette as stored; 0 to `OP2F_PLAYER_COLORS` - 1 draws that player's colour. With `aligned`, the canvas covers
/// every frame of the animation, so all its frames have one size and hotspot; otherwise it is cropped to this frame.
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_frame(
    sprites: *const Op2fSprites,
    animation: usize,
    frame: usize,
    player: i32,
    aligned: bool,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let frames = &index(&sprites.art.animations, animation, "animation")?.frames;
        index(frames, frame, "frame")?;
        let tint = tint(player)?;
        let sprite = if aligned {
            sprites.frame_rgba_aligned(animation, frame, tint)
        } else {
            sprites.frame_rgba(animation, frame, tint)
        };
        unsafe { put(out, Op2fImage(sprite.map_err(lib_err)?)) }
    })
}

/// The game's click test on a frame: whether a click at (`x`, `y`) lands on it, in frame coordinates (map pixel -
/// object pixel + the animation's displacement), with the original's quirks (Outpost2.exe 0x404F40).
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_hit(
    sprites: *const Op2fSprites,
    animation: usize,
    frame: usize,
    x: i32,
    y: i32,
    out: *mut bool,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let frames = &index(&sprites.art.animations, animation, "animation")?.frames;
        index(frames, frame, "frame")?;
        unsafe { put_value(out, sprites.hit(animation, frame, x, y) == Some(true)) }
    })
}

/// Which animation the game draws for a vehicle: `code` is its `vehicles.txt` code name (e.g. "LIGHT_TANK") or common
/// name ("Lynx", "ConVec"), `state` 0 for normal (1-3 are working states, falling back to 0 like the game), and
/// `rotation` its facing, 0-255 with 0 east, turning clockwise. Read from Outpost2.exe's unit types; see
/// op2-formats' units.rs.
///
/// # Safety
/// `code` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vehicle_animation(
    code: *const c_char,
    plymouth: bool,
    state: u32,
    rotation: u8,
    out: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let unit = units::vehicle(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no vehicle {code:?}")))?;
        let anim = unit
            .chassis(plymouth, state as usize, units::direction_index(rotation))
            .ok_or_else(|| fail(Op2fStatus::NotFound, format!("{code} has no animation for that state and direction")))?;
        unsafe { put_value(out, anim) }
    })
}

/// Which animation the game draws for a structure: `code` is its `building.txt` code name (e.g. "COMMAND"), `slot`
/// 0 the structure as built (its construction sequence, ending in the finished building), 1 and 2 damaged, 3
/// destroyed, 5-8 ambient animations. A slot the type doesn't have gives `OP2F_STATUS_NOT_FOUND`.
///
/// # Safety
/// `code` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_building_animation(code: *const c_char, plymouth: bool, slot: u32, out: *mut usize) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let art = units::building(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no structure {code:?}")))?;
        let anim = art.slots(plymouth).get(slot as usize).copied().filter(|&a| a != units::NONE);
        let anim = anim.ok_or_else(|| fail(Op2fStatus::NotFound, format!("{code} has no animation in slot {slot}")))?;
        unsafe { put_value(out, usize::from(anim)) }
    })
}

/// Draws a tank (`chassis` "LIGHT_TANK"/"Lynx", "MED_TANK"/"Panther" or "BIG_TANK"/"Tiger") with `weapon`'s turret (a `weapons.txt` code name, e.g. "LASER"), both facing `rotation`, as one
/// image with its hotspot on the unit's position. `player` as in `op2f_sprites_frame`.
///
/// # Safety
/// `sprites` is a live handle; `chassis` and `weapon` strings; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_tank(
    sprites: *const Op2fSprites,
    chassis: *const c_char,
    weapon: *const c_char,
    plymouth: bool,
    rotation: u8,
    player: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let chassis = unsafe { str_arg(chassis, "chassis") }?;
        let weapon = unsafe { str_arg(weapon, "weapon") }?;
        let unit = units::vehicle(chassis).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no vehicle {chassis:?}")))?;
        let turret = units::turret(weapon).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no weapon {weapon:?}")))?;
        let sprite = units::tank_sprite(sprites, unit, turret, plymouth, rotation, tint(player)?).map_err(lib_err)?;
        unsafe { put(out, Op2fImage(sprite)) }
    })
}

/// # Safety
/// `image` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_free(image: *mut Op2fImage) {
    unsafe { free(image) }
}

/// Width in pixels.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_width(image: *const Op2fImage) -> u32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.image.width)
}

/// Height in pixels.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_height(image: *const Op2fImage) -> u32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.image.height)
}

/// The pixel that sits on the object's map position: draw the image with its top-left corner at the object's pixel
/// position minus the hotspot.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_hotspot_x(image: *const Op2fImage) -> i32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.hotspot.0)
}

/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_hotspot_y(image: *const Op2fImage) -> i32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.hotspot.1)
}

/// The pixels, RGBA with straight (not premultiplied) alpha, row by row: width * height * 4 bytes, valid until the
/// image is freed. NULL for an empty image.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_rgba(image: *const Op2fImage) -> *const u8 {
    unsafe { image.as_ref() }.filter(|i| !i.0.image.rgba.is_empty()).map_or(std::ptr::null(), |i| i.0.image.rgba.as_ptr())
}

// ---- .vol archives ----

/// Opens a `.vol` archive by path. Entries are read from the file when asked for.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_open(path: *const c_char, out: *mut *mut Op2fVol) -> Op2fStatus {
    guard(|| {
        let path = unsafe { str_arg(path, "path") }?;
        let vol = VolArchive::open(path.as_ref()).map_err(lib_err)?;
        unsafe { put(out, Op2fVol(vol)) }
    })
}

/// Parses a `.vol` archive from memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_parse(data: *const u8, len: usize, out: *mut *mut Op2fVol) -> Op2fStatus {
    guard(|| {
        let vol = VolArchive::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, Op2fVol(vol)) }
    })
}

/// # Safety
/// `vol` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_free(vol: *mut Op2fVol) {
    unsafe { free(vol) }
}

/// Number of entries.
///
/// # Safety
/// `vol` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_entry_count(vol: *const Op2fVol) -> usize {
    unsafe { vol.as_ref() }.map_or(0, |v| v.0.entries().len())
}

/// Entry `index`'s file name.
///
/// # Safety
/// `vol` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_entry_name(vol: *const Op2fVol, index: usize, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        unsafe { copy_str(&self::index(vol.entries(), index, "entry")?.name, buf, cap, len) }
    })
}

/// Finds an entry by file name, ignoring case.
///
/// # Safety
/// `vol` is a live handle; `name` a string; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_find(vol: *const Op2fVol, name: *const c_char, index: *mut usize) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        let name = unsafe { str_arg(name, "name") }?;
        let found = vol.entries().iter().position(|e| e.name.eq_ignore_ascii_case(name));
        let i = found.ok_or_else(|| fail(Op2fStatus::NotFound, format!("{name} not in the archive")))?;
        unsafe { put_value(index, i) }
    })
}

/// Entry `index`'s contents, decompressed.
///
/// # Safety
/// `vol` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_read(vol: *const Op2fVol, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        let entry = self::index(vol.entries(), index, "entry")?;
        let data = vol.data(entry).map_err(lib_err)?.into_owned();
        unsafe { put(out, Op2fBytes(data)) }
    })
}
