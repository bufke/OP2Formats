# Ideas

Possible additions, not planned yet.

- Support `.OPM` files: reading and writing are in `op2-opm/` (2026-11-04). Still to do: merging a variant and a
  difficulty onto the master variant, `.opm` to map and in-game coordinates, unit names to unit art, and checking a
  mission against its map and tech tree.
- Support map `.JSON` files: in `op2-mapjson/` (2026-11-06), OP2MapJsonTools' PerRowPadded format, both ways.
- Support tile group `.JSON` files. The map JSON has no tile groups yet (OP2MapJsonTools' "TO ADD"); a `tileGroups`
  member would be kept by op2-mapjson as an unknown member but not turned into the `.map`'s tile groups.
