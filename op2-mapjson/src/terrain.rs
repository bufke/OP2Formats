//! A `.map`'s terrain type records: 264 bytes of little-endian 16-bit fields, as OP2Utility's `TerrainType` lays
//! them out (`src/Map/TerrainType.h`), which the JSON writes by name.

use serde::Deserialize;

/// A range of tile mapping indices, both ends included.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Range16 {
    pub start: u16,
    pub end: u16,
}

/// What a range of tile mappings turns into: bulldozed, rubble, tube, wall, lava and scorched graphics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerrainType {
    /// The tile mappings this terrain type covers.
    pub tile_range: Range16,
    /// The first of the bulldozed tile mappings, which follow on.
    pub bulldozed_tile_mapping_index: u16,
    /// The first of 4 common then 4 rare rubble tile mappings.
    pub rubble_tile_mapping_index: u16,
    pub tube_tile_mappings: [u16; 6],
    /// 5 sets of 16: lava, microbe, full-strength, damaged and heavily damaged walls.
    pub wall_tile_mapping_indexes: [[u16; 16]; 5],
    pub lava_tile_mapping_index: u16,
    pub flat1: u16,
    pub flat2: u16,
    pub flat3: u16,
    pub tube_tile_mapping_indexes: [u16; 16],
    pub scorched_tile_mapping_index: u16,
    pub scorched_range: [Range16; 3],
    pub unknown: [i16; 15],
}

/// Size of a terrain type record in a `.map`.
pub const TERRAIN_TYPE_BYTES: usize = 264;

impl TerrainType {
    /// The 16-bit fields in file order.
    fn words(&self) -> Vec<u16> {
        let mut w = vec![self.tile_range.start, self.tile_range.end, self.bulldozed_tile_mapping_index, self.rubble_tile_mapping_index];
        w.extend(self.tube_tile_mappings);
        w.extend(self.wall_tile_mapping_indexes.iter().flatten());
        w.extend([self.lava_tile_mapping_index, self.flat1, self.flat2, self.flat3]);
        w.extend(self.tube_tile_mapping_indexes);
        w.push(self.scorched_tile_mapping_index);
        w.extend(self.scorched_range.iter().flat_map(|r| [r.start, r.end]));
        w.extend(self.unknown.map(|v| v as u16));
        w
    }

    /// A record as a `.map` stores it.
    pub fn encode(&self) -> Vec<u8> {
        let out: Vec<u8> = self.words().into_iter().flat_map(u16::to_le_bytes).collect();
        debug_assert_eq!(out.len(), TERRAIN_TYPE_BYTES);
        out
    }

    /// Reads a record (`TERRAIN_TYPE_BYTES` bytes; a shorter one reads as if padded with zeros).
    pub fn decode(bytes: &[u8]) -> Self {
        let mut words = bytes.chunks(2).map(|c| u16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)])).chain(std::iter::repeat(0));
        let mut next = || words.next().expect("endless");
        let range = |next: &mut dyn FnMut() -> u16| Range16 { start: next(), end: next() };
        let tile_range = range(&mut next);
        let bulldozed_tile_mapping_index = next();
        let rubble_tile_mapping_index = next();
        let tube_tile_mappings = std::array::from_fn(|_| next());
        let wall_tile_mapping_indexes = std::array::from_fn(|_| std::array::from_fn(|_| next()));
        let lava_tile_mapping_index = next();
        let (flat1, flat2, flat3) = (next(), next(), next());
        let tube_tile_mapping_indexes = std::array::from_fn(|_| next());
        let scorched_tile_mapping_index = next();
        let scorched_range = std::array::from_fn(|_| range(&mut next));
        let unknown = std::array::from_fn(|_| next() as i16);
        Self {
            tile_range,
            bulldozed_tile_mapping_index,
            rubble_tile_mapping_index,
            tube_tile_mappings,
            wall_tile_mapping_indexes,
            lava_tile_mapping_index,
            flat1,
            flat2,
            flat3,
            tube_tile_mapping_indexes,
            scorched_tile_mapping_index,
            scorched_range,
            unknown,
        }
    }
}
