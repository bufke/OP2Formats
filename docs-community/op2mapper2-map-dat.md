# OP2Mapper2 placements (`.map.dat`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source read here is marked 2.2.4 in its project file). It edits `.map` files through the OP2Editor COM
library, and keeps the units and Gaia objects placed on a map in a file of its own beside it, named after the map with
`.dat` added (`eden04.map.dat`). The autosave's is `autosave.map.dat` in the mapper's folder.

**The game never reads it.** Outpost 2 takes a map's units from the mission DLL; a `.map` alone (see
[maps.md](../docs/maps.md)) is a complete terrain file. The `.map.dat` only lets the mapper show and edit the
placements and turn them into mission code with a [template](op2mapper2-tpl.md). No `.map.dat` file was available,
so the layout below is from the source only.

## Layout

Written with VB's binary file mode: little-endian, no padding.

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
  wrap ([maps.md](../docs/maps.md#coordinates)).
- A unit's `map_id` is an entry of [`units.ctl`](op2mapper2-units-ctl.md), its weapon one of
  [`weapons.ctl`](op2mapper2-weapons-ctl.md). An object's `map_id` and extras are an entry of
  [`objects.ctl`](op2mapper2-objects-ctl.md), copied from it when the object is placed.
- The flag is a VB `Boolean`; the loader reads the extras only when it equals −1. Other non-zero values are untested.
- The mapper writes the file whenever it saves the map (and to the autosave), only when the map has at least one
  record, deleting the old file first. Saving a map whose placements were all removed leaves the previous `.map.dat`
  in place.

## How the mapper loads it

Loading (`frmDocument.frm`, `LoadMap`) matches each record to a definition and replaces the record's type with it,
the unit or object by `map_id` (and extras) and then the weapon by `map_id`. An unknown `map_id` or weapon gives an
error message ("Map ID ... not found") and leaves that record unresolved.

Reading the source, the choice between matching by `map_id` alone and by `map_id` plus the extras is made from a
definition at a leftover loop index, not from the record's own flag. For the first record it is the list's first
entry (the shipped tables' Command Center, not Gaia); later it is wherever the previous record's lookups stopped:
that record's own definition when it had no weapon, else the entry whose position matches its weapon's place in the
weapon list. A beacon read in the `map_id`-only mode becomes the first entry with `map_id` 81, "Common Beacon 1".
After an unknown `map_id` with no weapon the index is past the end of the list, so the next record's lookup fails and
the whole load stops with "Map file failed to load". None of this was tried in the running mapper.

## Example

A Lynx with a laser for player 1 at column 10, row 5, then a rare beacon (two bars) at column 12, row 7:

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

## Writing one

Follow the layout above. It has no header beyond the count and no link to the map besides its name, so a tool
converting placements to or from it needs the map only for its size. Write each object's extras exactly as its
`objects.ctl` entry has them, since the mapper finds objects by them.

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `frmDocument.frm` (`LoadMap`, `SaveSelf`,
  `SaveSelfAs`, `SaveAutosave`), `modMain.bas` (the `UnitRec` and `UnitType` records), `modParsers.bas` (how the
  definition lists are filled).
