# Ideas

Possible additions, not planned yet. What is planned is in [NOTES.md](NOTES.md).

- Tile groups in JSON maps. The map JSON has no tile groups yet (OP2MapJsonTools' "TO ADD"); a `tileGroups` member
  would be kept by the `mapjson` module as an unknown member but not turned into the `.map`'s tile groups. A layout
  could reuse tile group JSON's (`mapjson::TileGroupJson`): one object per group with its header and rows.
- More of `.opm`: merging a variant and a difficulty onto the master variant, `.opm` to map and in-game coordinates,
  unit names to unit art, and checking a mission against its map and tech tree.
- Formats documented but not read: saved games (`docs/saved-games.md`), the story texts (`docs/story.md`), the
  settings files (`docs/settings.md`), OP2Lua missions (`docs-community/lua-missions.md`) and OP2Mapper2's files.
