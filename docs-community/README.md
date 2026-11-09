# Community file formats

File formats made by the Outpost 2 community's tools and projects, not by the original game: one document each,
with what the file is for, its layout field by field, which tools read and write it, and which part of OP2Formats
reads it. The game itself reads none of these files; the original game's formats are in [../docs/](../docs/README.md),
and these documents link there for the parts they share (tiles, cell types, coordinates, mission DLLs).

| Format | Files | Made by | Document | Read by |
|---|---|---|---|---|
| JSON maps | `.json` | OP2MapJsonTools | [map-json.md](map-json.md) | op2-mapjson |
| Tile group JSON | one tile group per `.json` | proposed, for map tools | [tile-group-json.md](tile-group-json.md) | not read yet |
| Missions | `.opm` | OP2MissionEditor, OP2DotNetMissionSDK | [opm.md](opm.md) | op2-opm |
| OP2Mapper2 unit table | `units.ctl` | OP2Mapper2 | [op2mapper2-units-ctl.md](op2mapper2-units-ctl.md) | not read |
| OP2Mapper2 weapon table | `weapons.ctl` | OP2Mapper2 | [op2mapper2-weapons-ctl.md](op2mapper2-weapons-ctl.md) | not read |
| OP2Mapper2 object table | `objects.ctl` | OP2Mapper2 | [op2mapper2-objects-ctl.md](op2mapper2-objects-ctl.md) | not read |
| OP2Mapper2 default terrains | `terrains.ctl` | OP2Mapper2 | [op2mapper2-terrains-ctl.md](op2mapper2-terrains-ctl.md) | not read |
| OP2Mapper2 placements | `<map>.map.dat` | OP2Mapper2 | [op2mapper2-map-dat.md](op2mapper2-map-dat.md) | not read |
| OP2Mapper2 code templates | `.tpl` | OP2Mapper2 | [op2mapper2-tpl.md](op2mapper2-tpl.md) | not read |
| OP2Lua missions | `.lua` + `.placement.lua` | OP2Lua | [lua-missions.md](lua-missions.md) | not read |

## Writing one

The rules in [../docs/README.md](../docs/README.md#writing-one) apply here too. In addition, name the tool or project
the format belongs to and the version the document was checked against, since community formats change with their
tools.
