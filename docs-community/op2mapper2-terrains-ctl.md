# OP2Mapper2 default terrains (`terrains.ctl`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source read here is marked 2.2.4 in its project file). `terrains.ctl` holds the terrain types it gives a
new map made with the default tilesets: for each terrain, its range of tiles and the tiles it uses for bulldozed
ground, rubble, walls, tubes, lava and scorch marks. The game never reads it; the mapper uses it only when it
creates a new map with the default tilesets, not for opening or saving maps.

## Where it is read from

At start-up the mapper asks OP2Editor's resource manager for `terrains.ctl`, which searches the game folder and its
`.vol` archives. If it is found there, it is copied to a temporary file in the mapper's folder (`opu*.tmp`), read
and deleted; if not, the copy in the mapper's own folder is read. A missing file, or one with `NumTerrains` below
1, gives the error "Couldn't read terrains.ctl" and new maps get no default terrains.

## Layout

An INI file, read with Windows' `GetPrivateProfileString`, so section and key names are case-insensitive and `;`
starts a comment line.

```ini
[terrains.ctl]
NumTerrains=4
[0]
StartTile=0
EndTile=437
Dozed=408
...
```

`[terrains.ctl]` holds `NumTerrains`. Then each terrain *i* is a section named `[i]` (0, 1, ...) with these keys,
read as integers; a missing key reads as 0:

| Keys | Count |
|---|---|
| `StartTile`, `EndTile` | 1 each |
| `Dozed`, `Rubble` | 1 each |
| `TubeUnk0`-`TubeUnk5` | 6 |
| `LavaWall0`-`15`, `MicrobeWall0`-`15`, `NormalWall0`-`15`, `DamagedWall0`-`15`, `RuinedWall0`-`15` | 16 each |
| `Lava`, `Flat1`, `Flat2`, `Flat3` | 1 each |
| `Tube0`-`Tube15` | 16 |
| `Scorched` | 1 |
| `Unknown0`-`Unknown20` | 21 |

That is 132 values, one per 16-bit field of the `.map`'s 264-byte terrain type record, in the record's order (see
[tilesets.md](../docs/tilesets.md#the-terrain-type-record)); `Unknown0`-`Unknown5` are the record's three scorch
ranges and `Unknown6`-`Unknown20` its 15 unknown words.

## The values are one lower than a map's

The values are tile mapping indices, but **one lower than the shipped maps'**: in the shipped file, `StartTile` and
`EndTile` of terrain 0 are 0 and 437 where the maps' record says 1 and 438, and the first normal wall is 330 where
the maps have 331. Measured: all 528 values of the shipped file's four terrains equal the matching words of
`eden04.map`'s terrain types minus 1, with the map's 0 written as 2011.

The reason is in how the mapper builds a new map (`frmDocument.frm`, `CreateMap`): it adds `well0000` as tileset 0,
maps in every tile of wells 1-12 in order, sets the terrains from `terrains.ctl`, and only then maps in the single
tile of `well0000`. OP2Editor's `MapInTiles` (`CTileSetManager.cpp`) appends to the mapping table, so wells 1-12 take
mappings 0-2010 (their 2011 tiles) and `well0000` takes 2011: the shipped maps' table shifted down by one, which is
what the values fit.

## Writing one

Copy the shipped file. To describe a map that uses the shipped maps' mappings, add 1 to every value (2011 becomes
0); to convert a map's terrain types into this file, subtract 1 (0 becomes 2011).

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `modMain.bas` (`ReadTerrainsCtl`,
  `GetSettingIniFromFile`), `frmDocument.frm` (`CreateMap`), and the shipped `terrains.ctl`.
- OP2Editor source (OutpostUniverse archive copy): `CTileSetManager.cpp` (`MapInTiles`).
- Measured: the shipped file's 528 values against `eden04.map`'s terrain type records (OPU 1.4.1's
  `OPU/base/maps/campaign`).
