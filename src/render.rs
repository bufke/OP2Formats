//! CPU composition of a map into an RGBA image, for previews and headless checks.

use crate::map::TileGroup;
use crate::{Error, GameFiles, Map, TILE_SIZE, Tileset};

/// Shown for tiles whose mapping or tileset can't be resolved.
const MISSING: [u8; 4] = [255, 0, 255, 255];

/// A map's tilesets, indexed like `Map::tilesets`. Failed loads are kept with their error so
/// callers can report them without aborting.
pub struct MapTilesets {
    pub sets: Vec<Option<Result<Tileset, Error>>>,
}

impl MapTilesets {
    pub fn load(files: &GameFiles, map: &Map) -> Self {
        let sets = map
            .tilesets
            .iter()
            .map(|slot| {
                slot.as_ref().map(|src| {
                    let (data, _) = files.read(&format!("{}.bmp", src.name))?;
                    Tileset::parse(&data)
                })
            })
            .collect();
        Self { sets }
    }

    pub fn get(&self, index: u16) -> Option<&Tileset> {
        self.sets.get(index as usize)?.as_ref()?.as_ref().ok()
    }

    /// RGBA pixels for the tile at map position (x, y), first animation frame.
    pub fn tile_pixels(&self, map: &Map, x: u32, y: u32) -> Option<&[u8]> {
        let mapping = map.mapping(map.tile(x, y))?;
        self.get(mapping.tileset)?.tile_rgba(mapping.graphic as u32)
    }
}

/// Renders a tile group at full resolution. Returns the image and how many cells could not be
/// resolved (drawn as `MISSING`).
pub fn tile_group(map: &Map, tilesets: &MapTilesets, group: &TileGroup) -> (Image, usize) {
    let (width, height) = (group.width * TILE_SIZE, group.height * TILE_SIZE);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut missing = 0;
    let row_bytes = (TILE_SIZE * 4) as usize;
    for (cell, &index) in group.mapping_indices.iter().enumerate() {
        let (cx, cy) = (cell as u32 % group.width, cell as u32 / group.width);
        let pixels = map
            .mappings
            .get(index as usize)
            .and_then(|m| tilesets.get(m.tileset)?.tile_rgba(m.graphic as u32));
        missing += pixels.is_none() as usize;
        for row in 0..TILE_SIZE {
            let dst = (((cy * TILE_SIZE + row) * width + cx * TILE_SIZE) * 4) as usize;
            match pixels {
                Some(p) => rgba[dst..dst + row_bytes].copy_from_slice(&p[row as usize * row_bytes..][..row_bytes]),
                None => rgba[dst..dst + row_bytes].as_chunks_mut::<4>().0.fill(MISSING),
            }
        }
    }
    (Image { width, height, rgba }, missing)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Renders the whole map at `px` pixels per tile (1..=32), box-filtering each 32x32 tile down.
pub fn compose(map: &Map, tilesets: &MapTilesets, px: u32) -> Image {
    let px = px.clamp(1, TILE_SIZE);
    let (width, height) = (map.width * px, map.height * px);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let block = TILE_SIZE / px;

    for ty in 0..map.height {
        for tx in 0..map.width {
            let tile = tilesets.tile_pixels(map, tx, ty);
            for oy in 0..px {
                for ox in 0..px {
                    let color = match tile {
                        Some(pixels) => average(pixels, ox * block, oy * block, block),
                        None => MISSING,
                    };
                    let i = (((ty * px + oy) * width + tx * px + ox) * 4) as usize;
                    rgba[i..i + 4].copy_from_slice(&color);
                }
            }
        }
    }
    Image { width, height, rgba }
}

fn average(tile: &[u8], x0: u32, y0: u32, block: u32) -> [u8; 4] {
    let mut sum = [0u32; 3];
    for y in y0..y0 + block {
        for x in x0..x0 + block {
            let i = ((y * TILE_SIZE + x) * 4) as usize;
            for c in 0..3 {
                sum[c] += tile[i + c] as u32;
            }
        }
    }
    let n = block * block;
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]
}
