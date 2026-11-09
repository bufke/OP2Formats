//! Engine-independent preparation of a map's graphics for a GPU renderer (op2-mapview, op2-engine). Every distinct
//! tile graphic the map uses becomes one layer of a texture array, so the GPU only needs a layer index per tile; a
//! low-resolution overview image covers zoomed-out views, where sampling 32px tiles directly would alias badly.

use std::collections::HashMap;

use crate::render::{Image, MapTilesets};
use crate::{Map, TILE_SIZE, Tile};

pub const LAYER_BYTES: usize = (TILE_SIZE * TILE_SIZE * 4) as usize;
/// Drawn for tiles whose mapping or graphic can't be resolved.
pub const MISSING_LAYER: u32 = 0;
/// Pixels per tile in the overview image.
pub const OVERVIEW_TILE: u32 = 8;

pub struct TilePalette {
    /// Layer for each mapping index; `None` if no tile uses the mapping or it can't be resolved.
    mapping_layer: Vec<Option<u32>>,
    /// RGBA, `LAYER_BYTES` per layer.
    pixels: Vec<u8>,
}

impl TilePalette {
    /// Collects the graphics of mappings the map's tiles actually use. Animation frames are not
    /// included: no shipped map places an animated mapping (see tools/README.md).
    pub fn build(map: &Map, tilesets: &MapTilesets) -> Self {
        let mut pixels = missing_tile();
        let mut mapping_layer = vec![None; map.mappings.len()];
        let mut graphic_layer = HashMap::new();
        for y in 0..map.height {
            for x in 0..map.width {
                let index = map.tile(x, y).mapping_index as usize;
                let Some(slot @ None) = mapping_layer.get_mut(index) else { continue };
                let m = &map.mappings[index];
                let Some(rgba) = tilesets.get(m.tileset).and_then(|t| t.tile(m.graphic.into())) else {
                    continue;
                };
                *slot = Some(*graphic_layer.entry((m.tileset, m.graphic)).or_insert_with(|| {
                    rgba.extend_rgba(&mut pixels);
                    (pixels.len() / LAYER_BYTES - 1) as u32
                }));
            }
        }
        Self { mapping_layer, pixels }
    }

    pub fn layer_count(&self) -> u32 {
        (self.pixels.len() / LAYER_BYTES) as u32
    }

    pub fn layer(&self, tile: Tile) -> u32 {
        self.mapping_layer.get(tile.mapping_index as usize).copied().flatten().unwrap_or(MISSING_LAYER)
    }

    /// Pixels of `count` consecutive layers starting at `first`.
    pub fn layers(&self, first: u32, count: u32) -> &[u8] {
        &self.pixels[first as usize * LAYER_BYTES..(first + count) as usize * LAYER_BYTES]
    }
}

/// Magenta and black checkerboard, so unresolved tiles are obvious at any zoom.
fn missing_tile() -> Vec<u8> {
    (0..TILE_SIZE * TILE_SIZE)
        .flat_map(|i| {
            let (x, y) = (i % TILE_SIZE, i / TILE_SIZE);
            if (x / 8 + y / 8) % 2 == 0 { [255, 0, 255, 255] } else { [0, 0, 0, 255] }
        })
        .collect()
}

/// Box-filters `src` down by `factor` in each direction (output at least 1x1; a trailing
/// partial block is dropped).
pub fn shrink(src: &Image, factor: u32) -> Image {
    let (width, height) = ((src.width / factor).max(1), (src.height / factor).max(1));
    let (fx, fy) = (factor.min(src.width), factor.min(src.height));
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            for sy in y * fy..(y + 1) * fy {
                for sx in x * fx..(x + 1) * fx {
                    let i = ((sy * src.width + sx) * 4) as usize;
                    for (s, &v) in sum.iter_mut().zip(&src.rgba[i..i + 4]) {
                        *s += v as u32;
                    }
                }
            }
            rgba.extend(sum.map(|s| (s / (fx * fy)) as u8));
        }
    }
    Image { width, height, rgba }
}

/// The whole map at `OVERVIEW_TILE` pixels per tile.
pub fn overview(map: &Map, palette: &TilePalette) -> Image {
    let thumbs: Vec<Image> = (0..palette.layer_count())
        .map(|layer| {
            let tile = Image { width: TILE_SIZE, height: TILE_SIZE, rgba: palette.layers(layer, 1).to_vec() };
            shrink(&tile, TILE_SIZE / OVERVIEW_TILE)
        })
        .collect();
    let (width, height) = (map.width * OVERVIEW_TILE, map.height * OVERVIEW_TILE);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let row_bytes = (OVERVIEW_TILE * 4) as usize;
    for ty in 0..map.height {
        for tx in 0..map.width {
            let thumb = &thumbs[palette.layer(map.tile(tx, ty)) as usize];
            for row in 0..OVERVIEW_TILE {
                let dst = (((ty * OVERVIEW_TILE + row) * width + tx * OVERVIEW_TILE) * 4) as usize;
                let src = row as usize * row_bytes;
                rgba[dst..dst + row_bytes].copy_from_slice(&thumb.rgba[src..src + row_bytes]);
            }
        }
    }
    Image { width, height, rgba }
}

/// Full mip chain for `base`, largest first, down to 1x1.
pub fn mip_chain(base: Image) -> Vec<Image> {
    let mut levels = vec![base];
    while let Some(last) = levels.last().filter(|l| l.width > 1 || l.height > 1) {
        let next = shrink(last, 2);
        levels.push(next);
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_averages_blocks() {
        let src = Image { width: 2, height: 2, rgba: vec![0, 0, 0, 255, 100, 0, 0, 255, 0, 200, 0, 255, 0, 0, 40, 255] };
        let out = shrink(&src, 2);
        assert_eq!((out.width, out.height), (1, 1));
        assert_eq!(out.rgba, vec![25, 50, 10, 255]);
    }

    #[test]
    fn mip_chain_reaches_one_pixel_for_non_square_images() {
        let base = Image { width: 8, height: 2, rgba: vec![9; 8 * 2 * 4] };
        let dims: Vec<_> = mip_chain(base).iter().map(|l| (l.width, l.height)).collect();
        assert_eq!(dims, vec![(8, 2), (4, 1), (2, 1), (1, 1)]);
    }

    /// Needs a game install; passes trivially without one.
    #[test]
    fn palette_reproduces_every_tile() {
        let Some(files) = crate::testdata::game_files() else { return };
        for name in ["cm01.map", "newworld.map"] {
            let (data, _) = files.read(name).unwrap();
            let map = Map::parse(&data).unwrap();
            let tilesets = MapTilesets::load(&files, &map);
            let palette = TilePalette::build(&map, &tilesets);
            for y in 0..map.height {
                for x in 0..map.width {
                    let expected = tilesets.tile_pixels(&map, x, y).unwrap();
                    assert_eq!(palette.layers(palette.layer(map.tile(x, y)), 1), expected.rgba(), "{name} ({x},{y})");
                }
            }
        }
    }
}
