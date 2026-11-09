//! Converts Outpost 2 `.map` files to and from the JSON maps of OP2MapJsonTools (and OP2MapViewerJson reads), in its
//! PerRowPadded layout: one line per row of tiles and of cell types, numbers padded into columns.
//!
//! ```text
//! header        width, height, map (the .map file's name), name, author, notes
//! tiles         height rows of width tile mapping indices
//! cellTypes     height rows of width cell types (op2-formats' CellType numbers, 0-31)
//! clipRect      x1, y1, x2, y2: only when it isn't the default (see default_clip_rect)
//! tileset       sources (filename, numTiles), tileMappings (tilesetIndex, tileGraphicIndex,
//!               animationCount, animationDelay) and terrainTypes (the .map's 264-byte records, decoded)
//! ```
//!
//! The JSON doesn't carry everything a `.map` has: the version tag, the empty tileset slots after the last used one,
//! the tile groups, and the bits of each tile word above the mapping index (unit index, lava, Blight; no shipped map
//! sets them). [`MapJson::to_map`] takes those from a base map when given one, else writes the shipped maps' values
//! (version 0x1011, 512 slots, no tile groups).

mod terrain;
mod write;

use op2_formats::Map;
use op2_formats::map::{TileMapping as MapTileMapping, TilesetSource};
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value};

pub use terrain::{Range16, TERRAIN_TYPE_BYTES, TerrainType};

/// The version tag the shipped maps have, used for a map made without a base.
pub const DEFAULT_VERSION: u32 = 0x1011;
/// The number of tileset slots the shipped maps have.
pub const DEFAULT_TILESET_SLOTS: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{0}")]
    Map(#[from] op2_formats::Error),
}

/// A JSON map.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapJson {
    pub header: Header,
    /// Tile mapping indices, `height` rows of `width`.
    pub tiles: Vec<Vec<u16>>,
    /// Cell types, `height` rows of `width`.
    pub cell_types: Vec<Vec<u8>>,
    /// The clip rectangle, when it isn't [`default_clip_rect`].
    pub clip_rect: Option<ClipRect>,
    pub tileset: Tileset,
    /// Members this library doesn't know, kept and written after `tileset`.
    #[serde(flatten)]
    pub extra: JsonMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    /// The `.map` file's name, e.g. "cm01.map".
    pub map: String,
    pub name: String,
    pub author: String,
    pub notes: String,
    #[serde(flatten)]
    pub extra: JsonMap<String, Value>,
}

/// The rectangle of the map the game shows and scrolls over, in the game's tile coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct ClipRect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tileset {
    /// The map's tileset slots up to the last one used; an empty `filename` is an empty slot.
    pub sources: Vec<Source>,
    pub tile_mappings: Vec<TileMapping>,
    pub terrain_types: Vec<TerrainType>,
    #[serde(flatten)]
    pub extra: JsonMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    /// The tileset's base name, e.g. "well0004".
    pub filename: String,
    pub num_tiles: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TileMapping {
    pub tileset_index: u16,
    pub tile_graphic_index: u16,
    pub animation_count: u16,
    pub animation_delay: u16,
}

/// The clip rectangle a map has unless set otherwise, and which the JSON leaves out: tiles 32 to 32 + width - 1
/// across (0 to width - 1 on a 512-wide map, which isn't padded), rows 0 to height - 2. 81 of the 89 shipped maps
/// have it.
pub fn default_clip_rect(width: u32, height: u32) -> ClipRect {
    let x = if width < 512 { 32 } else { 0 };
    ClipRect { x1: x, y1: 0, x2: x + width as i32 - 1, y2: height as i32 - 2 }
}

impl MapJson {
    /// Parses JSON map text (UTF-8, with or without a byte-order mark). Rows can be padded or not; the older flat
    /// arrays ("Original" format) aren't read.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
        for key in ["tiles", "cellTypes"] {
            if let Some(first) = value.get(key).and_then(|v| v.as_array()).and_then(|a| a.first())
                && !first.is_array()
            {
                return Err(Error::Invalid(format!(
                    "\"{key}\" is one flat array (OP2MapJsonTools' Original format); only rows (PerRow) are read"
                )));
            }
        }
        let json: Self = serde_json::from_value(value)?;
        json.check()?;
        Ok(json)
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::parse(&text)
    }

    /// The JSON text in the PerRowPadded layout: two-space indents, CRLF, one line per row of tiles (numbers padded
    /// to 4 characters) and cell types (2), as OP2MapJsonTools writes it.
    pub fn to_json(&self) -> String {
        write::per_row_padded(self)
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let path = path.as_ref();
        std::fs::write(path, self.to_json()).map_err(|source| Error::Io { path: path.display().to_string(), source })
    }

    /// Checks the grids match the header and hold values a `.map` can store.
    fn check(&self) -> Result<(), Error> {
        let (w, h) = (self.header.width as usize, self.header.height as usize);
        let bad = |s: String| Err(Error::Invalid(s));
        for (key, rows, max) in [
            ("tiles", self.tiles.iter().map(|r| r.iter().map(|&v| v as u32).collect::<Vec<_>>()).collect::<Vec<_>>(), 0x7FF),
            ("cellTypes", self.cell_types.iter().map(|r| r.iter().map(|&v| v as u32).collect()).collect(), 31),
        ] {
            if rows.len() != h {
                return bad(format!("\"{key}\" has {} rows; the header says the map is {h} tall", rows.len()));
            }
            for (y, row) in rows.iter().enumerate() {
                if row.len() != w {
                    return bad(format!("\"{key}\" row {y} has {} values; the header says the map is {w} wide", row.len()));
                }
                if let Some((x, v)) = row.iter().enumerate().find(|&(_, &v)| v > max) {
                    return bad(format!("\"{key}\" at ({x}, {y}) is {v}; the most a .map can hold is {max}"));
                }
            }
        }
        Ok(())
    }

    /// The JSON of a map. `map_file` goes in the header's `map` and, without its extension, `name` (as
    /// OP2MapJsonTools does); `author` and `notes` start empty.
    pub fn from_map(map: &Map, map_file: &str) -> Self {
        let (w, h) = (map.width, map.height);
        let tiles = (0..h).map(|y| (0..w).map(|x| map.tile(x, y).mapping_index).collect()).collect();
        let cell_types = (0..h).map(|y| (0..w).map(|x| (map.tile_bits(x, y) & 0x1F) as u8).collect()).collect();
        let [x1, y1, x2, y2] = map.clip_rect;
        let clip = ClipRect { x1, y1, x2, y2 };
        // Only the empty slots after the last used one are dropped (OP2Utility drops every empty slot, which would
        // move the tile mappings of a map with a gap onto other tilesets).
        let used = map.tilesets.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
        let sources = map.tilesets[..used]
            .iter()
            .map(|s| s.as_ref().map_or_else(Source::default, |s| Source { filename: s.name.clone(), num_tiles: s.tile_count }))
            .collect();
        let tile_mappings = map
            .mappings
            .iter()
            .map(|m| TileMapping {
                tileset_index: m.tileset,
                tile_graphic_index: m.graphic,
                animation_count: m.animation_count,
                animation_delay: m.animation_delay,
            })
            .collect();
        let name = std::path::Path::new(map_file).file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        Self {
            header: Header { width: w, height: h, map: map_file.to_string(), name, ..Default::default() },
            tiles,
            cell_types,
            clip_rect: (clip != default_clip_rect(w, h)).then_some(clip),
            tileset: Tileset {
                sources,
                tile_mappings,
                terrain_types: map.terrain_types.iter().map(|t| TerrainType::decode(t)).collect(),
                extra: JsonMap::new(),
            },
            extra: JsonMap::new(),
        }
    }

    /// The map this JSON describes. What the JSON doesn't carry (the version tag, the empty tileset slots after
    /// the last used one, tile groups, and the tile words' bits above the mapping index) comes from `base` when it
    /// is given (a map of the same size: the one the JSON came from, or a template), else from the shipped maps'
    /// defaults: version 0x1011, 512 slots, no tile groups, those bits clear.
    pub fn to_map(&self, base: Option<&Map>) -> Result<Map, Error> {
        self.check()?;
        let (w, h) = (self.header.width, self.header.height);
        if !(32..=1024).contains(&w) || !w.is_power_of_two() || !(1..=1024).contains(&h) {
            return Err(Error::Invalid(format!(
                "a {w}x{h} map can't be stored: the width must be a power of two from 32 to 1024, the height 1 to 1024"
            )));
        }
        if let Some(b) = base
            && (b.width, b.height) != (w, h)
        {
            return Err(Error::Invalid(format!("the base map is {}x{}, the JSON {w}x{h}", b.width, b.height)));
        }
        let mut map = Map::parse(&skeleton(base.map_or(DEFAULT_VERSION, |b| b.version), w, h))?;
        for y in 0..h {
            for x in 0..w {
                let high = base.map_or(0, |b| b.tile_bits(x, y) & !0xFFFF);
                let (m, c) = (self.tiles[y as usize][x as usize] as u32, self.cell_types[y as usize][x as usize] as u32);
                map.set_tile_bits(x, y, high | m << 5 | c);
            }
        }
        let clip = self.clip_rect.unwrap_or_else(|| default_clip_rect(w, h));
        map.clip_rect = [clip.x1, clip.y1, clip.x2, clip.y2];
        let slots = base.map_or(DEFAULT_TILESET_SLOTS, |b| b.tilesets.len()).max(self.tileset.sources.len());
        map.tilesets = (0..slots)
            .map(|i| {
                let s = self.tileset.sources.get(i)?;
                (!s.filename.is_empty()).then(|| TilesetSource { name: s.filename.clone(), tile_count: s.num_tiles })
            })
            .collect();
        map.mappings = self
            .tileset
            .tile_mappings
            .iter()
            .map(|m| MapTileMapping {
                tileset: m.tileset_index,
                graphic: m.tile_graphic_index,
                animation_count: m.animation_count,
                animation_delay: m.animation_delay,
            })
            .collect();
        map.terrain_types = self.tileset.terrain_types.iter().map(TerrainType::encode).collect();
        map.terrain_type_count = map.terrain_types.len();
        if let Some(b) = base {
            map.tile_groups = b.tile_groups.clone();
            map.tile_group_unknown = b.tile_group_unknown;
        }
        Ok(map)
    }
}

/// The bytes of an empty `.map` of the given size (no tilesets, mappings, terrain types or tile groups), for
/// `Map::parse` to make a `Map` from: op2-formats' `Map` has no constructor.
fn skeleton(version: u32, width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height) as usize * 4 + 64);
    for v in [version, 0, width.trailing_zeros(), height, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.resize(out.len() + (width * height) as usize * 4, 0);
    out.extend_from_slice(&[0; 16]); // clip rect
    out.extend_from_slice(b"TILE SET\x1a\0");
    for v in [0, 0, version, version, 0, 0] {
        out.extend_from_slice(&u32::to_le_bytes(v));
    }
    out
}
