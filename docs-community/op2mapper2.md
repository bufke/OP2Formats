# OP2Mapper2 files (`.ctl`, `.map.dat`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source measured here is marked 2.2.4 in its project file). It edits `.map` files through the
OP2Editor COM library and keeps a few files of its own:

| File | Where | What it holds |
|---|---|---|
| `units.ctl` | the mapper's folder | the player units it can place: structures and vehicles |
| `weapons.ctl` | the mapper's folder | turret weapons, and which units take one |
| `objects.ctl` | the mapper's folder | Gaia objects: walls, tubes, beacons, vents, fumaroles, wreckage |
| `terrains.ctl` | a `.vol` first, else the mapper's folder | the terrain types given to a new map |
| `<map file>.dat` | next to the map | the units and objects placed on that map |
| `*.tpl` | the mapper's folder | templates for the mission source code it generates |

**The game reads none of them.** Outpost 2 takes a map's units from the mission DLL; a `.map` alone (see
[maps.md](../docs/maps.md)) is a complete terrain file, and the `.map.dat` only lets the mapper show and edit the placements
and turn them into mission code. Everything here comes from reading the mapper's source (`modParsers.bas`,
`modMain.bas`, `frmDocument.frm`, `frmUnits.frm`) and its shipped `.ctl` files, checked against the game where noted.
No `.map.dat` file was available, so its layout is from the source only.

## The table files: `units.ctl`, `weapons.ctl`, `objects.ctl`

The three files share one line syntax and are read at start-up in this order: units, weapons, objects. Each line is
handled on its own:

1. Leading and trailing spaces are trimmed, then every tab is removed.
2. A line starting with `;` is a comment.
3. A line starting with `INCLUDE` (any case) reads another file with the same parser. The name is the text between
   the first pair of double quotes; it is taken relative to **the mapper's folder**, not to the including file.
4. In `weapons.ctl` only, a line starting with `HASTURRET` (any case) marks a unit as taking a turret (below).
5. An empty line is skipped.
6. Anything else is a record: comma-separated fields. Field 1 (counting from 0) is a string in double quotes; every
   other field is an integer made of digits and `-`. A space outside the quotes, a quote outside field 1, or any
   other character is an error that stops reading the file. Fields after the last one a table knows are ignored.

The shipped files use CRLF line ends and end with a comment line without a line break, which the files say works
around a parser bug. Numbers are converted with VB's `CLng` (32-bit); an empty number field (two commas in a row)
fails the conversion and stops reading the file.

Units and objects go into one definition list, units first; weapons into a second list.

### `units.ctl`

```
map_id,"name",art                                   a vehicle
map_id,"name",width,height,art                      a structure without tubes
map_id,"name",width,height,tube_x,tube_y,art        a structure with tubes
```

| Field | Meaning |
|---|---|
| map_id | the game's unit type id (the mission SDK's `map_id`), e.g. 1 Cargo Truck, 28 Command Center |
| name | the name shown in the unit list |
| width, height | footprint in tiles |
| tube_x, tube_y | where the tube connection lies, as an x and a y offset from the edge of the bulldozed border |
| art | the animation the mapper draws, an index into `op2_art.prt` (see [sprites.md](../docs/sprites.md)) |

The parser decides the kind by the number of fields: 3 is a vehicle, 5 a structure, 7 a structure with tubes. With 4
or 6 fields the entry has no art (−1) and isn't drawn. When placed, a vehicle or a structure without tubes sits with
its top-left tile on the clicked tile; a structure with tubes is drawn shifted left by tube_x − 1 and up by
tube_y − 1 tiles, so the clicked tile is its tube connection.

The shipped file has 53 records: 38 structures and 15 vehicles. Their `map_id`s are the ones the mission SDK's
`map_id` enumeration gives each name. The art values were checked against the animation tables in `Outpost2.exe`
(op2-formats `UnitArt`): each vehicle's art is in its Eden animation table (the first state, facing east, for 14
of the 15), and 35 of the 38 structures' art is the first animation slot of the Eden version of that building (the
building's construction animation; the mapper draws its last frame, the finished building). The other three, Guard
Post (441), Common Ore Mine (104) and Rare Ore Mine (405), aren't in those tables; the file itself says the Guard
Post has no turret-less art and borrows another picture. The footprints and tube offsets were not checked against
the game.

### `weapons.ctl`

```
map_id,"name",art
HASTURRET map_id
```

A weapon record gives the weapon's `map_id`, a name, and the turret animation drawn on top of the unit. All 11
shipped weapons' art is the weapon's Lynx turret facing east in `Outpost2.exe`'s tables (Eden's where the colonies
differ).

`HASTURRET` reads every digit after the keyword as one number and marks the unit with that `map_id` as taking a
turret. The unit must already be defined (so `units.ctl` must be read first), or reading stops with an error. The
shipped file marks the Guard Post (23), Tiger (7), Panther (6) and Lynx (5). The unit list then offers one entry per
weapon for each of these, named `unit/weapon`, and none without a weapon.

The shipped file gives Starflare 67 and Supernova 68, the Guard Post versions. The mission SDK's `map_id`
enumeration names 65 and 66 as the tanks' Starflare and Supernova, and the game agrees: its Vehicle Factory offers
tanks 65 and 66 while the Guard Post's weapons are 67 and 68 (measured in op2-rs; the factory's weapon list at
0x4E33D8). So a tank placed with either weapon gets the Guard Post's weapon id in the generated code. In the game,
types 67 and 68 (`TURRET_DESTRUCT`, `TURRET_DESTRUCT_ADV`, vtables 0x4D3A70 and 0x4D3AA0) have a Guard Post turret
routine only; their Tiger, Panther and Lynx routines are the base class's, which draw nothing (0x447190-0x4471B0).
Such a tank would show no turret. **Unconfirmed:** how it fights or detonates; a game started with one would settle
it.

### `objects.ctl`

```
map_id,"name",type,art[,extra1[,extra2[,extra3]]]
```

| Field | Meaning |
|---|---|
| map_id | the game's type id: 17 tube, 18 wall, 19 lava wall, 20 microbe wall, 81 mining beacon, 82 magma vent, 83 fumarole, 84 wreckage |
| name | the name shown in the Gaia list |
| type | which code template line the object uses: 0 beacon, 1 wall or tube, 2 wreck |
| art | an `op2_art.prt` animation, −1 for none, or a marker: −2 a white T, −3 to −5 the letters NW, LW and MW in cyan |
| extra1-3 | the values given to the template's `$extra1` to `$extra3`; −1 when left out |

The extras are the arguments of the mission SDK call the template writes:

| Type | Call | extra1 | extra2 | extra3 |
|---|---|---|---|---|
| 0 | `TethysGame::CreateBeacon` | 0 common, 1 rare | yield: 0 three bars, 1 two, 2 one | variant: 0-2, −1 random |
| 1 | `TethysGame::CreateWallOrTube` | the call's unused third argument | | |
| 2 | `TethysGame::CreateWreck` | the tech id the wreck gives | 1 if visible without a scout | |

The yield and variant values follow the mission SDK's `Yield` and `Variant` enumerations (−1 is random for both).
The shipped file has 13 records: a tube and three walls, six beacons that share `map_id` 81 and differ only in their
extras (common and rare, each at yield 2, 1 and 0, variant −1), the magma vent and the fumarole (type 0 with no
extras, so −1, −1, −1), and one wreck (tech 11999, visible).

Because several objects share a `map_id`, the mapper tells them apart by `map_id` and all three extras together.

## `terrains.ctl`

An INI file, read with Windows' profile functions. The mapper looks for it through OP2Editor's resource search (the
game's `.vol` archives) first and copies it to a temporary file, and falls back to its own folder.

```ini
[terrains.ctl]
NumTerrains=4
[0]
StartTile=0
EndTile=437
Dozed=408
...
```

`[terrains.ctl]` holds `NumTerrains`; the reading fails if it is below 1. Then each terrain *i* is a section named
`[i]` (0, 1, ...) with these keys, any missing key reading as 0:

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
[tilesets.md](../docs/tilesets.md#the-terrain-type-record)); `Unknown0`-`Unknown5` are the record's three scorch ranges and
`Unknown6`-`Unknown20` its 15 unknown words. The mapper uses them only when it creates a new map with the default
tilesets.

The values are mapping indices, but **one lower than the shipped maps'**: in the shipped file, `StartTile` and
`EndTile` of terrain 0 are 0 and 437 where the maps' record says 1 and 438, and the first normal wall is 330 where
the maps have 331. Measured: all 528 values of the shipped file's four terrains equal the matching words of
`eden04.map`'s terrain types minus 1, with the maps' 0 written as 2011. The reason is in how the mapper builds a new
map (`frmDocument.frm`, `CreateMap`): it adds `well0000` as tileset 0, maps in every tile of wells 1-12 in order,
sets the terrains from `terrains.ctl`, and only then maps in the single tile of `well0000`. OP2Editor's
`MapInTiles` (`CTileSetManager.cpp`) appends to the mapping table, so wells 1-12 take mappings 0-2010 (their 2011
tiles) and `well0000` 2011: the shipped maps' table shifted down by one, which is what the values fit.

## `.map.dat`

The placements for a map, in a file named after the map with `.dat` added (`eden04.map.dat`); the autosave is
`autosave.map.dat` in the mapper's folder. Written with VB's binary file mode: little-endian, no padding.

| Size | Type | Field |
|---|---|---|
| 4 | i32 | record count *n* |
| varies | | *n* records |

Each record:

| Size | Type | Field |
|---|---|---|
| 4 | i32 | x, map tile column (0 = the map's first column) |
| 4 | i32 | y, map tile row |
| 4 | i32 | player, 0-6; 0 for a Gaia object |
| 4 | i32 | `map_id` of the unit or object |
| 4 | i32 | weapon `map_id`; 0 for none |
| 2 | i16 | Gaia flag: −1 (0xFFFF) for an object from `objects.ctl`, 0 for a unit |
| 2 | i16 | extra1, only when the flag is −1 |
| 2 | i16 | extra2, only when the flag is −1 |
| 2 | i16 | extra3, only when the flag is −1 |

A unit record is 22 bytes and an object record 28, so the file is 4 + 22 × units + 28 × objects bytes. Records are
in the order they were placed.

- **Coordinates** are the file's tile coordinates, without the 32-column offset the game adds on maps that don't
  wrap ([maps.md](../docs/maps.md#coordinates)). The shipped code template converts them: it writes x + 32 and y. It adds
  the 32 on 512-wide maps too, where the game doesn't.
- The extras of an object are copied from its `objects.ctl` record when it is placed, so they repeat the definition.
- The mapper writes the file only when the map has at least one record, and deletes the old one first. Saving a map
  whose placements were all removed leaves the previous `.map.dat` in place.
- The flag is a VB `Boolean`; the loader reads the extras only when it equals −1. Other non-zero values are untested.

Loading (`frmDocument.frm`, `LoadMap`) matches each record to a definition and replaces the record's type with it.
Reading the source, the choice
between matching by `map_id` alone and by `map_id` plus the extras is made from a definition at a leftover loop
index, not from the record's own flag: for the first record it is the first definition (the Command Center, not
Gaia), and later it depends on where the previous record's unit and weapon lookups stopped (often the previous
record's own definition). A beacon read in the first mode becomes the first beacon in `objects.ctl` ("Common Beacon
1"). An unknown `map_id` gives an error message, leaves the record unresolved and leaves the index past the end of the
list, which can make the next record's lookup fail the whole load. None of this was tried in the running mapper.

Example, a Lynx with a laser for player 1 at column 10, row 5, then a rare beacon (two bars) at column 12, row 7:

```
02 00 00 00                count, 2
0A 00 00 00  05 00 00 00   x 10, y 5
01 00 00 00                player 1
05 00 00 00  3D 00 00 00   Lynx (5), Laser (61)
00 00                      not Gaia
0C 00 00 00  07 00 00 00   x 12, y 7
00 00 00 00                player 0
51 00 00 00  00 00 00 00   mining beacon (81), no weapon
FF FF                      Gaia
01 00  01 00  FF FF        rare, yield 1 (two bars), variant -1
```

## Making them

- A `.ctl` table: one record per line in the syntax above, no spaces outside the quoted name, a `;` comment as the last
  line to be safe. Keep `units.ctl` ahead of any `HASTURRET`.
- `terrains.ctl`: copy the shipped file; to describe a map with the shipped default mappings, add 1 to every value
  (2011 becomes 0).
- `.map.dat`: the layout above. It has no header beyond the count and no link to the map besides its name, so a tool
  converting placements to or from it needs the map only for its size.

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `modParsers.bas` (table parsers, code
  generator), `modMain.bas` (types, `ReadTerrainsCtl`), `frmDocument.frm` (`.map.dat` reading and writing, new maps),
  `frmUnits.frm` (players and unit list), `Default.tpl`, and its shipped `units.ctl`, `weapons.ctl`, `objects.ctl`
  and `terrains.ctl`.
- OP2Editor source (OutpostUniverse archive copy): `CTileSetManager.cpp` (`MapInTiles`).
- Disassembly of GOG's `Outpost2.exe` with an iced-x86 probe: the weapon types' vtables (turret routines).
- op2-rs (bufke's reimplementation): the weapon ids the game's Vehicle Factory and Guard Post use, measured.
- The Outpost 2 mission SDK headers (OutpostUniverse): the `map_id`, `Yield` and `Variant` enumerations and the
  `CreateBeacon`, `CreateWallOrTube` and `CreateWreck` calls.
- Measured: the `.ctl` art values against `Outpost2.exe`'s unit animation tables and `op2_art.prt` (2079 animations)
  with op2-formats `UnitArt` and `ArtFile`; `terrains.ctl` against `eden04.map`'s terrain types (OPU 1.4.1).
