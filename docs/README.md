# Outpost 2 file formats

The file formats of the original game, one document each: what the file is for, its layout byte by byte or field by
field, how the game uses it, and which part of OP2Formats reads it. Where OPU 1.4.1 (the community update) moves or
extends a file, the document says so. Facts are checked against the game's own files and its `Outpost2.exe`;
anything not yet settled is marked **unconfirmed**. The formats of the community's tools (JSON maps, `.opm`
missions, OP2Mapper2's files, OP2Lua missions) are in [../docs-community/](../docs-community/README.md).

| Format | Files | Document | Read by |
|---|---|---|---|
| Tilesets | `well0000.bmp`-`well0012.bmp` (BMP or PBMP) | [tilesets.md](tilesets.md) | op2-formats `tileset` |
| Maps | `*.map` | [maps.md](maps.md) | op2-formats `map` |
| Saved games | `*.op2` | [saved-games.md](saved-games.md) | not read |
| Volumes | `*.vol`, LZH compression | [volumes.md](volumes.md) | op2-formats `vol`, `lzh` |
| Sprites | `op2_art.prt` + `OP2_ART.BMP` | [sprites.md](sprites.md) | op2-formats `art`, `units` |
| Rule sheets | `vehicles.txt`, `weapons.txt`, `building.txt`, `space.txt`, `mines.txt`, `morale.txt` | [sheets.md](sheets.md) | op2-formats `sheets` |
| Tech trees | `edentek.txt`, `ply_tek.txt`, `multitek.txt`, `tutortek.txt` | [tech-trees.md](tech-trees.md) | op2-formats `sheets` |
| UI strings | the table in `Outpost2.exe`; OPU's `language.ini` | [strings.md](strings.md) | op2-formats `strings` |
| Music and sounds | `op2.clm`, `.wav` | [sound.md](sound.md) | op2-formats `sound` |
| Cutscenes | `.avi` (Cinepak, MS ADPCM) | [cutscenes.md](cutscenes.md) | op2-formats `avi`, `cinepak`, `adpcm` |
| The Blight's masks | `virmask.raw` | [virmask.md](virmask.md) | op2-formats `virmask` |
| Game tables | `Outpost2.exe` | [exe-tables.md](exe-tables.md) | op2-formats `exe` |
| Resources | `out2res.dll` and the other DLLs | [resources.md](resources.md) | op2-formats `pe_res` |
| Mission DLLs | `c*.dll`, `e*.dll`, `p*.dll`, `ml*.dll`, ... and OPU's mission packs | [mission-dlls.md](mission-dlls.md) | not read |
| Story texts | `story.vol`: briefings, objectives and story `.rtf`, `ch_title.txt`, `credits.txt` | [story.md](story.md) | not read |
| Settings | `outpost2.ini` (and OPU's), OPU's `base/ui` | [settings.md](settings.md) | not read |

Not covered: files that aren't Outpost 2's own formats, such as the help file (`.chm`), the TrueType fonts and PNG in
OPU's `base/ui` (standard formats; `settings.md` says what loads them), installer and uninstaller files, and logs.

## Writing one

- Each document is complete on its own: everything needed to read and write the format is here, not behind a link to
  another site.
- Say where each fact comes from: OP2Formats' code, a measurement (what was run on which files), or the disassembly
  (with the address). Credit the projects whose work a document builds on, such as OP2Utility, by name.
- Write in your own words; don't copy other documents.
- No game data in the documents beyond the small numbers a format needs (counts, indices, tags); no extracted images.
