# JSON maps (OP2MapJsonTools)

A JSON map is a `.map` file written out as text, for tools rather than the game: OP2MapJsonTools makes and reads
them, OP2MapViewerJson draws them, and anyone can edit or diff them in a text editor. The game never reads one; a
JSON map has to be converted back to a `.map` (see [maps.md](../docs/maps.md)) before it can be played. Read, written and
converted by op2-mapjson, which writes the files OP2MapJsonTools wrote back byte for byte.

Unless a section says otherwise, the numbers here were measured with op2-mapjson on the 89 maps of GOG Outpost 2
with OPU 1.4.1, and on two files OP2MapJsonTools wrote: `map1.json` (a 128×128 custom map) and `eden03.json` (the
campaign map).

## Layouts

OP2MapJsonTools has three ways of writing the same members. They differ only in how `tiles` and `cellTypes` are
laid out:

| Layout | `tiles` and `cellTypes` | Written by op2-mapjson | Read by op2-mapjson |
|---|---|---|---|
| Original | one flat array each, `width × height` values, row after row | no | no (refused with an error) |
| PerRow | an array of `height` rows, each row on one line | no | yes |
| PerRowPadded | as PerRow, with the numbers right-aligned in columns | yes | yes |

PerRow and PerRowPadded are the same JSON; only the spaces differ. This document describes PerRowPadded, the one
both sample files use. A reader tells Original apart by the first element of `tiles` or `cellTypes`: a number
there, not an array, means flat arrays.

## The file

UTF-8 text. op2-mapjson writes no byte-order mark and accepts one. The root object has these members, in this
order:

| Member | Type | Contents |
|---|---|---|
| `header` | object | size and descriptive text |
| `tiles` | array of rows | the tile mapping index of every cell |
| `cellTypes` | array of rows | the cell type of every cell |
| `clipRect` | object | the clip rectangle; only present when it isn't the default |
| `tileset` | object | the tileset slots, the tile mappings and the terrain types |

Members are read by name, so their order doesn't matter to a reader. Members op2-mapjson doesn't know are kept and
written back (see [Unknown members](#unknown-members)).

### `header`

| Member | Type | Contents |
|---|---|---|
| `width` | integer | width in tiles: 32 to 1024, a power of two (64 to 512 in the shipped maps) |
| `height` | integer | height in tiles, 1 to 1024 (64 to 256 in the shipped maps) |
| `map` | string | the `.map` file's name, e.g. `eden03.map` |
| `name` | string | a display name |
| `author` | string | free text |
| `notes` | string | free text |

When op2-mapjson converts a map it puts the given file name in `map`, the name without its extension in `name`,
and leaves `author` and `notes` empty. Both OP2MapJsonTools files have the file name with its extension in `name`
(`map1.map`, `eden03.map`), `Unknown` in `author` and `N/A` in `notes`: its front end, OP2JsonMap, passes the map's
file name and those two fixed strings for every map it exports (`fMain.vb`, `ExportMapToJson`). Nothing reads these four strings back into the `.map`, which has no place for them.

### `tiles`

`height` arrays of `width` integers. Row `y`, element `x` is the tile mapping index of cell (x, y): bits 5-15 of
the cell's tile word, 0 to 2047 (see [maps.md](../docs/maps.md#tile-words)). The value selects an entry of
`tileset.tileMappings`, not a tile in a well directly.

The rows are in plain row order, top row first. The `.map` stores its tile words in 32-column strips
([maps.md](../docs/maps.md#tile-order)); a converter reorders them. The coordinates are the file's: `tiles[0][0]` is the
map's top-left cell, without the 32-column padding the game adds on maps that don't wrap.

The shipped maps use mapping indices up to 2011; `map1.json` uses one up to 2013 (one of the default table's spare
mappings that show tile 0).

### `cellTypes`

`height` arrays of `width` integers, laid out like `tiles`: the cell type of each cell, 0 to 31, the low 5 bits of
the tile word. The numbers are op2-formats' `CellType` values, listed in [maps.md](../docs/maps.md#cell-types). The shipped
maps use 0 to 11.

### `clipRect`

| Member | Type |
|---|---|
| `x1`, `y1`, `x2`, `y2` | signed 32-bit integers |

The `.map`'s clip rect, in the game's coordinates (game x = file x + 32 on maps that don't wrap). The member is
left out when the clip rect equals the default for the map's size, and a missing `clipRect` means that default:

| Map | Default x1 | y1 | x2 | y2 |
|---|---|---|---|---|
| narrower than 512 | 32 | 0 | 32 + width − 1 | height − 2 |
| 512 wide | 0 | 0 | 511 | height − 2 |

81 of the 89 shipped maps have the default and their JSON has no `clipRect`. The other eight carry it: `cm03`,
`cm04` and `eden08`, whose clip rects narrow the top or bottom, and all five wrapping maps (`fl01`, `mp6_01`,
`newworld`, `survatw`, `survaxen`).

The default for 512-wide maps is a quirk. No shipped wrapping map uses (0, 0, 511, height − 2): all five store
x1 = −1 and x2 = 2147483647 (no east or west edge), with y1 = 1 or 2 and y2 = 254. So the JSON of a wrapping map
always carries `clipRect`, and a hand-made 512-wide JSON map without one gets a clip rect that no shipped map has.
Give a wrapping map an explicit `clipRect` copied from a shipped one.

### `tileset`

| Member | Type | Contents |
|---|---|---|
| `sources` | array of objects | the `.map`'s tileset slots, up to the last one in use |
| `tileMappings` | array of objects | the tile mappings, in mapping index order |
| `terrainTypes` | array of objects | the terrain type records |

#### `sources`

| Member | Type | Contents |
|---|---|---|
| `filename` | string | the well's name without an extension, e.g. `well0004` |
| `numTiles` | integer | the well's tile count |

Element *i* is tileset slot *i*. An empty slot before the last used one is written as `filename` `""` and
`numTiles` 0, so the slot numbers in `tileMappings` stay right. The empty slots after the last used one are left
out. Every shipped map has 13 entries, `well0000` to `well0012` (see [tilesets.md](../docs/tilesets.md)).

#### `tileMappings`

| Member | Type | Contents |
|---|---|---|
| `tilesetIndex` | integer, 0-65535 | the slot (index into `sources`) |
| `tileGraphicIndex` | integer, 0-65535 | the tile within that well |
| `animationCount` | integer, 0-65535 | how many following tiles the mapping cycles through (0 = still) |
| `animationDelay` | integer, 0-65535 | time between frames |

Element *k* is mapping index *k*. 87 shipped maps have the default table of 2017 mappings; `CK9_2v2ai` and `Swarm`
have 2012. See [tilesets.md](../docs/tilesets.md#how-a-map-uses-the-tiles).

#### `terrainTypes`

One object per 264-byte terrain type record of the `.map` (4 in every shipped map). The members, in file order,
with OP2Utility's names; every number is 16-bit, unsigned except `unknown`:

| Member | Type | Bytes in the record |
|---|---|---|
| `tileRange` | object `{ start, end }` | 4 |
| `bulldozedTileMappingIndex` | integer | 2 |
| `rubbleTileMappingIndex` | integer | 2 |
| `tubeTileMappings` | array of 6 integers | 12 |
| `wallTileMappingIndexes` | array of 5 arrays of 16 integers | 160 |
| `lavaTileMappingIndex` | integer | 2 |
| `flat1`, `flat2`, `flat3` | integers | 6 |
| `tubeTileMappingIndexes` | array of 16 integers | 32 |
| `scorchedTileMappingIndex` | integer | 2 |
| `scorchedRange` | array of 3 objects `{ start, end }` | 12 |
| `unknown` | array of 15 signed integers (−32768 to 32767) | 30 |

What the fields mean, and their values in the default table, is in [tilesets.md](../docs/tilesets.md#terrain-types). The
five wall sets are lava, microbe, normal, damaged and heavily damaged walls.

## Text layout

PerRowPadded is Newtonsoft.Json's indented output with the grids squeezed onto one line per row. Exactly:

- Two spaces per indent level. Each member is `"name": value` (one space after the colon) on its own line; members
  are separated by a comma at the end of the line.
- An object opens with `{` at the end of the member's line and closes with `}` on its own line at the member's
  indent. An object that is an array element starts on its own line, indented one level deeper than the array's
  member.
- Lines end in CRLF. The file ends with the root's `}` and no line break after it.
- An empty array is `[]` and an empty object `{}`, on the member's line.
- The plain number lists (`tubeTileMappings`, `tubeTileMappingIndexes`, `unknown`) have one number per line.
- Each row of `tiles`, of `cellTypes` and of `wallTileMappingIndexes` is one line: `[ `, the numbers joined by
  `, `, then ` ]`. Within a row, every number is right-aligned (padded with spaces on the left) to the same width:
  the widest number in that row, but at least 4 characters in `tiles`, 2 in `cellTypes` and 1 in
  `wallTileMappingIndexes`. Each row is padded on its own, so columns line up between rows only when the rows have
  the same widest number. With mapping indices below 10000, every `tiles` row is padded to 4.
- Strings use JSON's escapes (`\"`, `\\`, control characters); other characters are written as they are, in UTF-8.

The start of a 32×2 map, for illustration:

```
{
  "header": {
    "width": 32,
    "height": 2,
    "map": "x.map",
    "name": "x",
    "author": "",
    "notes": ""
  },
  "tiles": [
    [    0,   17,  439, 1206, ... ],
    [    5,    5,    5,    5, ... ]
  ],
  "cellTypes": [
    [  0,  7, 11,  4, ... ],
    ...
```

and a terrain type's walls:

```
        "wallTileMappingIndexes": [
          [ 270, 271, 272, ... ],
          ...
        ],
```

A shipped map's JSON is 332 KB (64×64) to 1.6 MB (512×256); most of it is the 2017 tile mappings, at five lines
each.

### Unknown members

Members op2-mapjson doesn't know, in the root, `header` or `tileset`, are read as plain JSON, kept, and written
back at the end of their object, after the known members, in alphabetical order of their names. They are written
in the same Newtonsoft layout, every array one element per line. A file that has unknown members in another order
comes back with the same content but those members reordered.

## What the JSON doesn't carry

A JSON map has the terrain and the tileset; four things in a `.map` have no member:

- the version tag (0x1011 in every shipped map);
- the empty tileset slots after the last used one (the shipped maps have 512 slots, 13 used);
- the tile groups (218 in 87 shipped maps) and the unknown word before them;
- the tile words' bits above the mapping index: unit index, lava, lava possible, Blight and wall bits. No shipped
  map sets any of them.

## Converting

**`.map` to JSON** (op2-mapjson `MapJson::from_map`): the tiles and cell types are unpacked from the tile words in
row order; `clipRect` is written only when it isn't the default; `sources` takes the slots up to the last used
one; mappings and terrain types are copied as they are.

**JSON to `.map`** (`MapJson::to_map`). The JSON is checked first: `tiles` and `cellTypes` must have `height` rows
of `width` values, tiles at most 2047 and cell types at most 31, the width a power of two from 32 to 1024 and the
height 1 to 1024. What the JSON doesn't carry comes from a **base map** when one is given, which must be the same
size (the map the JSON was made from, or a template): its version tag, its number of tileset slots (or the JSON's,
if more), its tile groups, and bits 16-31 of each of its tile words. Without a base, the shipped maps' values: tag
0x1011, 512 slots, no tile groups, those bits clear. The tile mappings and terrain types always come from the JSON.

Measured: every shipped map converts to JSON, reads back the same, writes the same text, and converts back to its
original bytes with itself as the base; without a base, to the same map apart from the four things above. Both
OP2MapJsonTools files write back byte for byte, and op2-mapjson's conversion of `eden03.map`, given the file's
three header strings, is `eden03.json` byte for byte.

### Differences from OP2MapJsonTools

- OP2MapJsonTools' export to `.map` starts from a blank template of one of ten sizes and keeps the template's
  mappings and terrain types, so a JSON map whose mappings differ from the template's comes out drawn wrongly.
  op2-mapjson uses the JSON's own, and takes any size the checks above allow.
- OP2Utility, which OP2MapJsonTools uses, drops every empty tileset slot when trimming them; with a gap among the
  used slots this shifts later slots down and points mappings at the wrong wells. op2-mapjson drops only the empty
  slots at the end. Maps without gaps, which includes every shipped map, give the same JSON either way.

## Making one

- Start from a converted shipped map of the size you want, or write the five members by hand.
- Keep `tiles` within the mappings the file lists; with the default 2017 mappings, 0-2011 are tiles and 2012-2016
  show tile 0 of `well0000`.
- Cell types 0-11 for the ground; the game sets the others as it plays.
- For a 512-wide map write `clipRect` explicitly (see above).
- Convert with a base map when one exists, so its tile groups and slot count carry over.

## Sources

- op2-mapjson `lib.rs` (members, checks, conversion), `write.rs` (text layout), `terrain.rs` (terrain type
  records), its README and `tests/mapjson.rs`.
- op2-formats `map.rs` (the `.map` side).
- OP2MapJsonTools (Leviathan): the three layouts and the two sample files it wrote; its own format notes; its front
  end OP2JsonMap's `fMain.vb` (the strings it fills in).
- OP2Utility (OutpostUniverse): `TerrainType.h` field names, `TrimTilesetSources`.
- Measured: all 89 shipped maps of OPU 1.4.1 converted with op2-mapjson; `map1.json` and `eden03.json` read and
  written back.
