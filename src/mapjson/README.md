# op2-formats: `mapjson`

Converts Outpost 2 `.map` files to and from the JSON maps of
[OP2MapJsonTools](https://github.com/leviathan400/OP2MapJsonToolsLibrary), which OP2MapViewerJson and other tools
read. Only its PerRowPadded layout is written (one line per row of tiles and of cell types, numbers in columns);
rows read padded or not. The older flat arrays ("Original" format) aren't read.

Behind the `mapjson` feature: `op2-formats = { version = "0.2", features = ["mapjson"] }`.

```rust
let map = files.read_map("cm01.map")?;
let mut json = op2_formats::mapjson::MapJson::from_map(&map, "cm01.map");
json.header.author = "Dynamix".into();
json.save("cm01.json")?;

let json = op2_formats::mapjson::MapJson::open("cm01.json")?;
std::fs::write("cm01.map", json.to_map(None)?.write())?;
```

## The format

| Member | What |
|---|---|
| `header` | `width`, `height`, `map` (the `.map` file's name), `name`, `author`, `notes` |
| `tiles` | `height` rows of `width` tile mapping indices (0-2047), padded to 4 characters |
| `cellTypes` | `height` rows of cell types (0-31, op2-formats' `CellType` numbers), padded to 2 |
| `clipRect` | `x1`, `y1`, `x2`, `y2`, only when it isn't the default: x1 32 (0 on a 512-wide map), y1 0, x2 x1 + width - 1, y2 height - 2. 81 of the 89 shipped maps have the default |
| `tileset.sources` | the tileset slots up to the last used one: `filename`, `numTiles` |
| `tileset.tileMappings` | `tilesetIndex`, `tileGraphicIndex`, `animationCount`, `animationDelay` |
| `tileset.terrainTypes` | the `.map`'s 264-byte terrain type records, by OP2Utility's field names (`TerrainType.h`) |

The text is Newtonsoft.Json's indented layout (two spaces, CRLF, no line break at the end), with each row of
`tiles`, `cellTypes` and `wallTileMappingIndexes` on one line, `[ a, b ]`, its numbers right-aligned to the widest
in the row (at least 4 characters in `tiles`, 2 in `cellTypes`). A file OP2MapJsonTools wrote reads and writes back
byte for byte. Members this library doesn't know are kept and written back.

## What the JSON doesn't carry

The version tag, the empty tileset slots after the last used one, the tile groups, and the bits of each tile word
above the mapping index (unit index, lava, Blight; no shipped map sets any). `MapJson::to_map(Some(&base))` takes
them from a base map of the same size, such as the map the JSON came from; `to_map(None)` uses the shipped maps'
values: version 0x1011, 512 slots, no tile groups, those bits clear. Every shipped map converts to JSON and back to
the same `.map` bytes with itself as the base.

## Tile group JSON

`TileGroupJson` reads and writes one tile group per file, the format OP2Mapper3 and OP2TileSetTools use
([tile-group-json.md](../../docs-community/tile-group-json.md)), in the same layout, and converts to and from a `.map`'s
tile groups:

```rust
let mut map = files.read_map("cm01.map")?;
for group in op2_formats::mapjson::map_tile_groups(&map)? {
    group.save(format!("{}.json", group.name))?;
}
let group = op2_formats::mapjson::TileGroupJson::open("snowroc9.json")?;
map.tile_groups.push(group.to_map_group());
```

Reading takes rows padded or not, the older flat `tiles` array, missing header texts, and unknown members (written
back); the rows set the size. Every value must be a mapping index, 0 to 2047.

## Differences from OP2MapJsonTools

- JSON to `.map` uses the JSON's own tile mappings and terrain types, and works for any width that is a power of two
  from 32 to 1024. OP2MapJsonTools' `ExportMapFile` starts from a blank template of one of ten sizes and keeps the
  template's mappings and terrain types, so a map whose mappings differ from the template's draws wrongly.
- `.map` to JSON drops only the empty tileset slots after the last used one. OP2Utility's `TrimTilesetSources`
  drops every empty slot, which would point the tile mappings of a map with a gap at the wrong tilesets. Maps
  without gaps (all the shipped ones) give the same JSON either way.

## Tests

`tests/mapjson.rs`: with `OP2_DIR`, every shipped map converts to JSON text, reads back, writes the same text, and
converts to the same `.map` bytes; without a base, to the same map but for what the JSON doesn't carry. With
`OP2_MAPJSON` naming a file OP2MapJsonTools wrote, or a folder of them, each writes back byte for byte; when its
`map` is a game map (and `OP2_DIR` is set), our own conversion of that map is the file byte for byte, and the file
converts back to the map's bytes. Checked with `map1.json` (a 128x128 custom map) and `eden03.json` (the campaign
map, OPU's copy). Plus the error cases and the terrain type encoding. The sample files aren't committed: one is a
conversion of a game map.
