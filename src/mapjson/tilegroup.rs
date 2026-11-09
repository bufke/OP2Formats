//! Tile group JSON: one tile group per file, as OP2Mapper3 and OP2TileSetTools read and write it
//! (`docs-community/tile-group-json.md`). A `header` object (width, height, tileset, name, bmp, notes) and `tiles`,
//! one row of tile mapping indices per line, in the PerRowPadded layout of the JSON maps.

use serde_json::{Map as JsonMap, Value};

use super::Error;
use crate::Map;
use crate::map::TileGroup;

/// The largest tile mapping index a tile word holds (11 bits).
pub const MAX_MAPPING_INDEX: u16 = 2047;

/// The tileset prefix the game's own tiles have (`well0000.bmp` ..).
pub const DEFAULT_TILESET: &str = "well00";

/// A tile group JSON file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TileGroupJson {
    /// The tileset the group is drawn from, as its 6-character prefix (`well00` for the game's).
    pub tileset: String,
    /// The group's name.
    pub name: String,
    /// The file name of a picture of the group (`<name>.bmp` beside the `.json`), empty when there is none.
    pub bmp: String,
    pub notes: String,
    /// Rows of tile mapping indices, top to bottom; every row the same length. The group's size is this grid's.
    pub tiles: Vec<Vec<u16>>,
    /// `header` members this library doesn't know, written back.
    pub header_extra: JsonMap<String, Value>,
    /// Root members this library doesn't know, written back.
    pub extra: JsonMap<String, Value>,
}

impl TileGroupJson {
    /// Parses tile group JSON (UTF-8, with or without a byte-order mark). Rows are read padded or not, and the older
    /// form with `tiles` as one flat array of `width × height` values, row by row, is read too (OP2TileSetTools
    /// still reads it). When `header`'s `width` and `height` disagree with the rows, the rows win, as in OP2Mapper3.
    /// Header members that are missing read as empty text.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let bad = |s: String| Error::Invalid(s);
        let Value::Object(mut root) = serde_json::from_str(text.trim_start_matches('\u{feff}'))? else {
            return Err(bad("a tile group is a JSON object".into()));
        };
        let mut header = match root.remove("header") {
            Some(Value::Object(h)) => h,
            Some(_) => return Err(bad("\"header\" isn't an object".into())),
            None => JsonMap::new(),
        };
        let mut text_member = |key: &str| -> Result<String, Error> {
            match header.remove(key) {
                None | Some(Value::Null) => Ok(String::new()),
                Some(Value::String(s)) => Ok(s),
                Some(other) => Err(bad(format!("header \"{key}\" isn't text: {other}"))),
            }
        };
        let (tileset, name, bmp, notes) = (text_member("tileset")?, text_member("name")?, text_member("bmp")?, text_member("notes")?);
        let mut size_member = |key: &str| -> Result<Option<usize>, Error> {
            match header.remove(key) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v.as_u64().map(|n| Some(n as usize)).ok_or_else(|| bad(format!("header \"{key}\" isn't a size: {v}"))),
            }
        };
        let (width, height) = (size_member("width")?, size_member("height")?);

        let cell = |v: &Value| -> Result<u16, Error> {
            v.as_u64()
                .filter(|&n| n <= MAX_MAPPING_INDEX as u64)
                .map(|n| n as u16)
                .ok_or_else(|| bad(format!("tile {v} isn't a mapping index (0 to {MAX_MAPPING_INDEX})")))
        };
        let tiles: Vec<Vec<u16>> = match root.remove("tiles") {
            Some(Value::Array(items)) if items.first().is_some_and(Value::is_array) => items
                .iter()
                .map(|row| row.as_array().ok_or_else(|| bad("\"tiles\" mixes rows and numbers".into()))?.iter().map(cell).collect())
                .collect::<Result<_, _>>()?,
            Some(Value::Array(items)) if !items.is_empty() => {
                let (Some(w), Some(h)) = (width, height) else {
                    return Err(bad("flat \"tiles\" needs the header's width and height".into()));
                };
                if w == 0 || items.len() != w * h {
                    return Err(bad(format!("flat \"tiles\" has {} values for {w}×{h}", items.len())));
                }
                let flat = items.iter().map(cell).collect::<Result<Vec<_>, _>>()?;
                flat.chunks(w).map(<[u16]>::to_vec).collect()
            }
            Some(Value::Array(_)) => return Err(bad("\"tiles\" is empty".into())),
            Some(_) => return Err(bad("\"tiles\" isn't an array".into())),
            None => return Err(bad("no \"tiles\"".into())),
        };
        let w = tiles[0].len();
        if w == 0 || tiles.iter().any(|r| r.len() != w) {
            return Err(bad("every row of \"tiles\" must hold the same number of values, at least one".into()));
        }
        Ok(Self { tileset, name, bmp, notes, tiles, header_extra: header, extra: root })
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::parse(&text)
    }

    /// Width in tiles.
    pub fn width(&self) -> usize {
        self.tiles.first().map_or(0, Vec::len)
    }

    /// Height in tiles.
    pub fn height(&self) -> usize {
        self.tiles.len()
    }

    /// The text OP2Mapper3 and OP2TileSetTools write: two-space indents, CRLF, no line break at the end, the six
    /// header members in order, each row of `tiles` on one line with its numbers right-aligned to the row's widest
    /// and at least 4 characters.
    pub fn to_json(&self) -> String {
        super::write::tile_group(self)
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let path = path.as_ref();
        std::fs::write(path, self.to_json()).map_err(|source| Error::Io { path: path.display().to_string(), source })
    }

    /// A `.map`'s tile group as JSON. Its values must be mapping indices (no shipped map's are larger than 2011); a
    /// group whose name isn't text (11 shipped maps carry stale bytes there) gets the lossy text.
    pub fn from_map_group(group: &TileGroup, tileset: &str) -> Result<Self, Error> {
        let (w, h) = (group.width as usize, group.height as usize);
        if w == 0 || h == 0 || group.mapping_indices.len() != w * h {
            return Err(Error::Invalid(format!("tile group {} is {w}×{h} with {} values", group.name, group.mapping_indices.len())));
        }
        let tiles = group
            .mapping_indices
            .chunks(w)
            .map(|row| {
                row.iter()
                    .map(|&v| u16::try_from(v).ok().filter(|&v| v <= MAX_MAPPING_INDEX))
                    .collect::<Option<Vec<u16>>>()
                    .ok_or_else(|| Error::Invalid(format!("tile group {} holds a value above {MAX_MAPPING_INDEX}", group.name)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { tileset: tileset.into(), name: group.name.clone(), tiles, ..Default::default() })
    }

    /// The group as a `.map` tile group: its name and the rows as 32-bit words. `tileset`, `bmp` and `notes` have no
    /// place in a `.map`.
    pub fn to_map_group(&self) -> TileGroup {
        TileGroup {
            name: self.name.clone(),
            name_bytes: self.name.as_bytes().to_vec(),
            width: self.width() as u32,
            height: self.height() as u32,
            mapping_indices: self.tiles.iter().flatten().map(|&v| v as u32).collect(),
        }
    }

    /// The highest mapping index the group uses: placing it on a map needs that many mappings, plus one.
    pub fn max_mapping_index(&self) -> u16 {
        self.tiles.iter().flatten().copied().max().unwrap_or(0)
    }
}

/// The tileset prefix of a map, as the tools write it in a tile group's `tileset`: the first 6 characters of its first
/// tileset slot that is in use (`well00` for the shipped maps). `None` when no slot is in use.
pub fn tileset_prefix(map: &Map) -> Option<String> {
    map.tilesets.iter().flatten().next().map(|s| s.name.chars().take(6).collect())
}

/// Every tile group of a map as JSON, with the map's [`tileset_prefix`]. A map with no tile groups gives none.
pub fn map_tile_groups(map: &Map) -> Result<Vec<TileGroupJson>, Error> {
    let tileset = tileset_prefix(map).unwrap_or_else(|| DEFAULT_TILESET.into());
    map.tile_groups.iter().map(|g| TileGroupJson::from_map_group(g, &tileset)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout OP2Mapper3 writes, with CRLF.
    const SNOWROC9: &str = "{\r\n  \"header\": {\r\n    \"width\": 2,\r\n    \"height\": 3,\r\n    \"tileset\": \"well00\",\r\n    \"name\": \"snowroc9\",\r\n    \"bmp\": \"\",\r\n    \"notes\": \"\"\r\n  },\r\n  \"tiles\": [\r\n    [ 1934, 1935 ],\r\n    [ 1936, 1937 ],\r\n    [ 1938, 1939 ]\r\n  ]\r\n}";

    #[test]
    fn reads_and_writes_back_the_tools_layout() {
        let g = TileGroupJson::parse(SNOWROC9).unwrap();
        assert_eq!((g.width(), g.height(), g.name.as_str(), g.tileset.as_str()), (2, 3, "snowroc9", "well00"));
        assert_eq!(g.tiles, vec![vec![1934, 1935], vec![1936, 1937], vec![1938, 1939]]);
        assert_eq!(g.to_json(), SNOWROC9);
        assert_eq!(g.max_mapping_index(), 1939);
    }

    #[test]
    fn pads_each_row_to_its_widest_and_at_least_four() {
        let g = TileGroupJson { name: "x".into(), tiles: vec![vec![1, 22, 333], vec![57, 0, 2047]], ..Default::default() };
        let text = g.to_json();
        assert!(text.contains("\r\n    [    1,   22,  333 ],\r\n    [   57,    0, 2047 ]\r\n  ]\r\n}"), "{text}");
        assert!(text.contains("\"width\": 3,\r\n    \"height\": 2,"), "{text}");
        assert!(!text.ends_with('\n'));
    }

    #[test]
    fn reads_leniently() {
        // The old flat form, unpadded rows, a BOM, missing header text, unknown members kept.
        let flat = "\u{feff}{\"header\":{\"width\":3,\"height\":2,\"name\":\"f\",\"author\":\"me\"},\"tiles\":[1,2,3,4,5,6],\"x\":1}";
        let g = TileGroupJson::parse(flat).unwrap();
        assert_eq!(g.tiles, vec![vec![1, 2, 3], vec![4, 5, 6]]);
        assert_eq!((g.tileset.as_str(), g.bmp.as_str()), ("", ""));
        assert_eq!(g.header_extra["author"], "me");
        assert_eq!(g.extra["x"], 1);
        let again = TileGroupJson::parse(&g.to_json()).unwrap();
        assert_eq!(again, g, "written back with the unknown members");

        // The rows win over the header's size.
        let g = TileGroupJson::parse("{\"header\":{\"width\":9,\"height\":9},\"tiles\":[[1,2]]}").unwrap();
        assert_eq!((g.width(), g.height()), (2, 1));
    }

    #[test]
    fn refuses_what_isnt_a_tile_group() {
        for (text, why) in [
            ("[]", "object"),
            ("{\"header\":{}}", "no \"tiles\""),
            ("{\"tiles\":[[1,2],[3]]}", "same number"),
            ("{\"tiles\":[[2048]]}", "mapping index"),
            ("{\"tiles\":[[-1]]}", "mapping index"),
            ("{\"tiles\":[1,2,3]}", "width and height"),
            ("{\"header\":{\"width\":2,\"height\":2},\"tiles\":[1,2,3]}", "3 values for 2×2"),
            ("{\"header\":{\"name\":5},\"tiles\":[[1]]}", "isn't text"),
        ] {
            let e = TileGroupJson::parse(text).unwrap_err().to_string();
            assert!(e.contains(why), "{text}: {e}");
        }
    }

    #[test]
    fn converts_to_and_from_a_map_group() {
        let group = TileGroup { name: "mesa".into(), name_bytes: b"mesa".to_vec(), width: 3, height: 2, mapping_indices: vec![10, 11, 12, 13, 14, 15] };
        let g = TileGroupJson::from_map_group(&group, "well00").unwrap();
        assert_eq!(g.tiles, vec![vec![10, 11, 12], vec![13, 14, 15]]);
        let back = g.to_map_group();
        assert_eq!((back.width, back.height, &back.mapping_indices, &back.name_bytes), (3, 2, &group.mapping_indices, &group.name_bytes));

        let too_big = TileGroup { mapping_indices: vec![4000], width: 1, height: 1, ..group.clone() };
        assert!(TileGroupJson::from_map_group(&too_big, "well00").is_err());
        let wrong_size = TileGroup { width: 2, height: 2, ..group };
        assert!(TileGroupJson::from_map_group(&wrong_size, "well00").is_err());
    }
}
