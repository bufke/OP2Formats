# OP2Mapper2 weapon table (`weapons.ctl`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source read here is marked 2.2.4 in its project file). `weapons.ctl`, in the mapper's own folder, lists
the turret weapons it offers and marks which units take one. The game never reads it.

It is read at start-up after [`units.ctl`](op2mapper2-units-ctl.md) and before
[`objects.ctl`](op2mapper2-objects-ctl.md). Weapons go into a list of their own; a placement in a
[`.map.dat`](op2mapper2-map-dat.md) names its weapon by `map_id`.

## Line syntax

The same as the other two tables. Each line is handled on its own:

1. Leading and trailing spaces are trimmed, then every tab is removed.
2. A line starting with `;` is a comment.
3. A line starting with `INCLUDE` (any case) reads another file with the same parser. The name is the text between
   the first pair of double quotes, taken relative to **the mapper's folder**, not to the including file.
4. A line starting with `HASTURRET` (any case) marks a unit as taking a turret (below).
5. An empty line is skipped.
6. Anything else is a record: comma-separated fields. Field 1 (counting from 0) is a string in double quotes; it
   can't hold a double quote. Every other field is an integer made of the digits and `-`. A space outside the
   quotes, a quote outside field 1 or any other character is an error that stops reading the file; so is a field
   that doesn't convert to a number. Fields past the third are ignored, whatever they hold.

The records read before an error stay in the list. The shipped file uses CRLF line ends and ends with a comment line
without a line break, which the file says works around a parser bug.

## Records

```
map_id,"name",art
HASTURRET map_id
```

| Field | Type | Meaning |
|---|---|---|
| map_id | 32-bit | the weapon's `map_id` in the game |
| name | text | the name shown in the unit list |
| art | 32-bit | the turret animation drawn on top of the unit, an index into `op2_art.prt` (see [sprites.md](../docs/sprites.md)) |

`HASTURRET` reads every digit after the keyword (anything else is skipped) as one number and marks the unit with that
`map_id` as taking a turret. The unit must already be defined, so `units.ctl` comes first; an unknown `map_id`, or no
digits at all, stops reading the file with an error. The unit list then offers one entry per weapon for each marked
unit, named `unit/weapon`, and none without a weapon.

## The shipped file

11 weapons: Laser 61, Microwave 62, Sticky Foam 71, Rail Gun 63, EMP 60, RPG 64, Starflare 67, ESG 70, Supernova
68, Acid Cloud 59 and Thor's Hammer 72. `HASTURRET` marks the Guard Post (23), Tiger (7), Panther (6) and Lynx (5).
All 11 weapons' art is the weapon's Lynx turret facing east in `Outpost2.exe`'s tables (Eden's where the colonies
differ).

Starflare and Supernova are given as 67 and 68, the Guard Post's versions. The mission SDK's `map_id` enumeration
names 65 and 66 as the tanks' Starflare and Supernova, and the game agrees: its Vehicle Factory offers tanks 65 and
66, while the Guard Post's weapons are 67 and 68 (measured in op2-rs; the factory's weapon list at 0x4E33D8). So a
tank placed with either weapon gets the Guard Post's weapon id in the generated code. In the game, types 67 and 68
(`TURRET_DESTRUCT`, `TURRET_DESTRUCT_ADV`, vtables 0x4D3A70 and 0x4D3AA0) have a Guard Post turret routine only;
their Tiger, Panther and Lynx routines are the base class's, which draw nothing (0x447190-0x4471B0). Such a tank would
show no turret. **Unconfirmed:** how it fights or detonates; a game started with one would settle it.

## Writing one

One record per line, no spaces outside the quoted name, `HASTURRET` lines naming units `units.ctl` defines, and a
`;` comment as the last line to be safe. Use 65 and 66 for tank Starflares and Supernovas and 67 and 68 for the
Guard Post's.

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `modParsers.bas` (`ParseWeaponTable`),
  `frmUnits.frm` (the unit list), and the shipped `weapons.ctl`.
- The Outpost 2 mission SDK headers (OutpostUniverse): the `map_id` enumeration.
- Disassembly of GOG's `Outpost2.exe` with an iced-x86 probe: the weapon types' vtables (turret routines).
- op2-rs (bufke's reimplementation): the weapon ids the game's Vehicle Factory and Guard Post use, measured.
- Measured: the art values against `Outpost2.exe`'s unit animation tables with op2-formats `UnitArt`.
