//! `.map` files (terrain only; units come from mission DLLs). Layout follows OP2Utility's
//! MapReader:
//!
//! ```text
//! header        u32 version, i32 is_saved_game, u32 lg_width, u32 height, u32 tileset_slots
//! tiles         width*height packed u32, stored in 32-tile-wide column strips (see tile_index)
//! clip rect     4 x i32
//! tilesets      tileset_slots x (u32-len name, then u32 tile count if name non-empty)
//! "TILE SET\x1a"
//! mappings      u32 count, count x 8 bytes
//! terrain types u32 count, count x 264 bytes (kept raw, not decoded)
//! version tag   twice
//! tile groups   u32 count, u32 unknown, then per group: u32 w, u32 h, w*h u32, u32-len name
//! ```

use crate::Error;
use crate::reader::Reader;

const MIN_VERSION: u32 = 0x1010;
const TERRAIN_TYPE_SIZE: usize = 264;

/// Movement/terrain class of a tile. Discriminants match the game's 5-bit field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellType {
    FastPassible1,
    Impassible2,
    SlowPassible1,
    SlowPassible2,
    MediumPassible1,
    MediumPassible2,
    Impassible1,
    FastPassible2,
    NorthCliffs,
    CliffsHighSide,
    CliffsLowSide,
    VentsAndFumaroles,
    DozedArea,
    Rubble,
    NormalWall,
    MicrobeWall,
    LavaWall,
    Tube,
    /// Unused padding values 12-20.
    Padding(u8),
}

impl CellType {
    pub fn from_bits(bits: u8) -> Self {
        use CellType::*;
        match bits {
            0 => FastPassible1,
            1 => Impassible2,
            2 => SlowPassible1,
            3 => SlowPassible2,
            4 => MediumPassible1,
            5 => MediumPassible2,
            6 => Impassible1,
            7 => FastPassible2,
            8 => NorthCliffs,
            9 => CliffsHighSide,
            10 => CliffsLowSide,
            11 => VentsAndFumaroles,
            21 => DozedArea,
            22 => Rubble,
            23 => NormalWall,
            24 => MicrobeWall,
            25 => LavaWall,
            26..=31 => Tube,
            other => Padding(other),
        }
    }

    /// Coarse passability bucket, for overlays.
    pub fn speed(self) -> Speed {
        use CellType::*;
        match self {
            FastPassible1 | FastPassible2 | DozedArea | Tube => Speed::Fast,
            MediumPassible1 | MediumPassible2 | Rubble => Speed::Medium,
            SlowPassible1 | SlowPassible2 => Speed::Slow,
            VentsAndFumaroles => Speed::Special,
            _ => Speed::Impassable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Fast,
    Medium,
    Slow,
    Special,
    Impassable,
}

#[derive(Debug, Clone, Copy)]
pub struct Tile {
    pub cell_type: CellType,
    pub mapping_index: u16,
    pub lava: bool,
    pub lava_possible: bool,
    /// Bit 29: the Blight may spread from this tile (the game sets it on a 2x2 block's corner with Blight in it).
    pub spreading: bool,
    pub microbe: bool,
    pub wall_or_building: bool,
}

impl Tile {
    pub fn from_bits(v: u32) -> Self {
        Self {
            cell_type: CellType::from_bits((v & 0x1F) as u8),
            mapping_index: ((v >> 5) & 0x7FF) as u16,
            // bits 16..27: unit index (always empty in map files)
            lava: v & (1 << 27) != 0,
            lava_possible: v & (1 << 28) != 0,
            spreading: v & (1 << 29) != 0,
            microbe: v & (1 << 30) != 0,
            wall_or_building: v & (1 << 31) != 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TilesetSource {
    /// Base name without extension, e.g. `well0004`.
    pub name: String,
    pub tile_count: u32,
}

/// Which tileset image and frame a tile draws.
#[derive(Debug, Clone, Copy)]
pub struct TileMapping {
    pub tileset: u16,
    pub graphic: u16,
    pub animation_count: u16,
    pub animation_delay: u16,
}

#[derive(Debug, Clone)]
pub struct TileGroup {
    /// Name as text (lossy). Unnamed groups in some maps hold stale non-text bytes.
    pub name: String,
    /// The stored name bytes, written back verbatim.
    pub name_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub mapping_indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Map {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    /// Indexed by tileset number; empty slots are `None`.
    pub tilesets: Vec<Option<TilesetSource>>,
    pub mappings: Vec<TileMapping>,
    pub terrain_type_count: usize,
    /// Terrain type records, kept raw (264 bytes each, not decoded) so the map can be rewritten.
    pub terrain_types: Vec<Vec<u8>>,
    /// Clip rect as stored (x1, y1, x2, y2).
    pub clip_rect: [i32; 4],
    pub tile_groups: Vec<TileGroup>,
    /// Second word of the tile group header (meaning unknown), kept for rewriting.
    pub tile_group_unknown: u32,
    /// Packed tile words in file order (see `tile_index`); `Tile::from_bits` decodes one.
    tiles: Vec<u32>,
}

impl Map {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        let version = r.u32()?;
        if version < MIN_VERSION {
            return Err(Error::Invalid(format!("map version tag {version:#x} is below {MIN_VERSION:#x}")));
        }
        if r.i32()? != 0 {
            return Err(Error::Unsupported("saved games (.op2) are not supported, only .map files".into()));
        }
        let lg_width = r.u32()?;
        let height = r.u32()?;
        if lg_width > 10 || height == 0 || height > 1024 {
            return Err(Error::Invalid(format!("implausible map size: lg_width {lg_width}, height {height}")));
        }
        let width = 1 << lg_width;
        let tileset_slots = r.u32()?;

        let tile_count = (width * height) as usize;
        let tiles = (0..tile_count).map(|_| r.u32()).collect::<Result<_, _>>()?;
        let clip_rect = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];

        let mut tilesets = Vec::with_capacity(tileset_slots as usize);
        for _ in 0..tileset_slots {
            let name = r.len_string()?;
            tilesets.push(if name.is_empty() {
                None
            } else {
                Some(TilesetSource { name, tile_count: r.u32()? })
            });
        }

        if r.bytes(10)? != b"TILE SET\x1a\0" {
            return Err(Error::Invalid("'TILE SET' marker not found after tileset list".into()));
        }

        let mapping_count = r.u32()? as usize;
        let mut mappings = Vec::with_capacity(mapping_count);
        for _ in 0..mapping_count {
            mappings.push(TileMapping {
                tileset: r.u16()?,
                graphic: r.u16()?,
                animation_count: r.u16()?,
                animation_delay: r.u16()?,
            });
        }

        let terrain_type_count = r.u32()? as usize;
        let terrain_types =
            (0..terrain_type_count).map(|_| r.bytes(TERRAIN_TYPE_SIZE).map(<[u8]>::to_vec)).collect::<Result<_, _>>()?;

        for _ in 0..2 {
            let tag = r.u32()?;
            if tag != version {
                return Err(Error::Invalid(format!("version tag {tag:#x} does not match header {version:#x}")));
            }
        }

        let group_count = r.u32()?;
        let tile_group_unknown = r.u32()?;
        let mut tile_groups = Vec::with_capacity(group_count as usize);
        for _ in 0..group_count {
            let width = r.u32()?;
            let height = r.u32()?;
            let mapping_indices = (0..width * height).map(|_| r.u32()).collect::<Result<_, _>>()?;
            let len = r.u32()? as usize;
            let name_bytes = r.bytes(len)?.to_vec();
            let name = String::from_utf8_lossy(&name_bytes).into_owned();
            tile_groups.push(TileGroup { name, name_bytes, width, height, mapping_indices });
        }

        Ok(Self {
            version,
            width,
            height,
            tilesets,
            mappings,
            terrain_type_count,
            terrain_types,
            clip_rect,
            tile_groups,
            tile_group_unknown,
            tiles,
        })
    }

    /// Serializes the map in the `.map` layout `parse` reads. Parsing a shipped map and writing
    /// it back reproduces the file byte for byte (tested on every map).
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.tiles.len() * 4 + 4096);
        let u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
        let string = |out: &mut Vec<u8>, s: &str| {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        };
        // Tileset names are plain ASCII (well0000 ...); group names are kept as raw bytes.
        u32(&mut out, self.version);
        u32(&mut out, 0);
        u32(&mut out, self.width.trailing_zeros());
        u32(&mut out, self.height);
        u32(&mut out, self.tilesets.len() as u32);
        for &t in &self.tiles {
            u32(&mut out, t);
        }
        for v in self.clip_rect {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for slot in &self.tilesets {
            match slot {
                Some(t) => {
                    string(&mut out, &t.name);
                    u32(&mut out, t.tile_count);
                }
                None => u32(&mut out, 0),
            }
        }
        out.extend_from_slice(b"TILE SET\x1a\0");
        u32(&mut out, self.mappings.len() as u32);
        for m in &self.mappings {
            for v in [m.tileset, m.graphic, m.animation_count, m.animation_delay] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        u32(&mut out, self.terrain_types.len() as u32);
        for t in &self.terrain_types {
            out.extend_from_slice(t);
        }
        u32(&mut out, self.version);
        u32(&mut out, self.version);
        u32(&mut out, self.tile_groups.len() as u32);
        u32(&mut out, self.tile_group_unknown);
        for g in &self.tile_groups {
            u32(&mut out, g.width);
            u32(&mut out, g.height);
            for &i in &g.mapping_indices {
                u32(&mut out, i);
            }
            u32(&mut out, g.name_bytes.len() as u32);
            out.extend_from_slice(&g.name_bytes);
        }
        out
    }

    /// Tiles are stored in vertical strips 32 tiles wide, each strip `height` rows tall.
    fn tile_index(&self, x: u32, y: u32) -> usize {
        (((x >> 5) * self.height + y) * 32 + (x & 31)) as usize
    }

    pub fn tile(&self, x: u32, y: u32) -> Tile {
        Tile::from_bits(self.tiles[self.tile_index(x, y)])
    }

    /// The packed 32-bit tile word (cell type bits 0-4, mapping 5-15, unit index 16-26, flags).
    pub fn tile_bits(&self, x: u32, y: u32) -> u32 {
        self.tiles[self.tile_index(x, y)]
    }

    pub fn set_tile_bits(&mut self, x: u32, y: u32, bits: u32) {
        let i = self.tile_index(x, y);
        self.tiles[i] = bits;
    }

    /// The mapping a tile uses, if its index is in range (bad indices show as missing tiles).
    pub fn mapping(&self, tile: Tile) -> Option<&TileMapping> {
        self.mappings.get(tile.mapping_index as usize)
    }

    pub fn tileset_name(&self, index: u16) -> Option<&str> {
        self.tilesets.get(index as usize)?.as_ref().map(|t| t.name.as_str())
    }

    /// 512-wide maps wrap around east/west and have no coordinate padding in-game.
    pub fn wraps(&self) -> bool {
        self.width >= 512
    }

    /// Internal (mission/game) tile x minus file tile x: the game pads non-wrapping maps with 32 tiles on the west
    /// (`GameMap::GetPaddingWidth()`), wrapping maps not at all. Internal y = file y. Verified against the running
    /// game's whole cell grid on cm01 and newworld (harness/oracle/FINDINGS.md).
    pub fn x_padding(&self) -> i32 {
        if self.wraps() { 0 } else { 32 }
    }
}
