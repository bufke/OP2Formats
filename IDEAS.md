# Ideas

Possible additions, not planned yet.

- Support `.OPM` files: reading and writing are in op2-formats' `opm` module (2026-11-04). Still to do: merging a variant and a
  difficulty onto the master variant, `.opm` to map and in-game coordinates, unit names to unit art, and checking a
  mission against its map and tech tree.
- Support map `.JSON` files: in op2-formats' `mapjson` module (2026-11-06), OP2MapJsonTools' PerRowPadded format, both ways.
- Tile group `.JSON` files: read and written by `mapjson::TileGroupJson` (2026-11-10), OP2Mapper3's and
  OP2TileSetTools' format. The map JSON itself has no tile groups yet (OP2MapJsonTools' "TO ADD"); a `tileGroups`
  member would be kept by the `mapjson` module as an unknown member but not turned into the `.map`'s tile groups.
