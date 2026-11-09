# Mission DLLs (`.dll`)

Every Outpost 2 level is a Windows DLL: a 32-bit PE file whose exports describe the level (its name, map, tech tree,
type and player count) and whose code sets it up and runs it through the game's mission API. The game reads the
description straight out of the file to build its menus, then loads the DLL with `LoadLibrary` when a game starts.
This document covers what the game needs from the file: the names, the exports and their layouts, how and when
each is used, and the save-region contract. The mission API itself (units, triggers, groups) is not covered.

Unless a section says otherwise, the numbers were measured on GOG Outpost 2 with OPU 1.4.1 by a probe that reads
each DLL's PE headers and export table (all 284 `.dll` files in the game folder and under `OPU`), and the behaviour
comes from the raw disassembly of GOG's `Outpost2.exe` 1.2.0.7 and of `OP2Shell.dll`, with addresses.

## Where they are

| Folder | Mission DLLs | Notes |
|---|---|---|
| game folder | 106 | the Sierra missions: campaigns, tutorials, colony games, demos, multiplayer |
| `OPU\base\maps\<mission>\` | 106 | byte-identical copies of the game folder's 106, one folder per map (`campaign`, `tutorial`, `misc`, `PieChart`, ...) |
| `OPU\maps\<pack>\` | 53 | community missions shipped with OPU, plus the OP2Lua missions (see [lua-missions.md](../docs-community/lua-missions.md)) |

That is 159 distinct files (the 106 copies are identical by hash; no two of the 53 are). A DLL is a mission when it
exports `DescBlock`; 16 other DLLs in the install don't (the shell, `out2res.dll`, network and loader DLLs, OPU's
`OPUPatch.dll`, `NetHelper.dll`, `OP2LuaCore.dll`, and `op2extra.dll`, a helper library that three packs ship and
their missions import).

## File names and mission types

`DescBlock`'s first field is the mission type: a campaign mission's number (1 to 12) or one of these negative values.
The file name prefixes are the convention the original game's menus rely on.

| Type | Value | Prefix | How the stock game lists it |
|---|---|---|---|
| Campaign mission *n* | 1 to 12 | `e`, `p` | not listed: the game builds the name `E%02d.DLL` or `P%02d.DLL` from the colony and mission number (0x48940B, 0x48772D) |
| Colony game | −1 | `c` | Colony Games dialog: `c*.dll`, type must be −1 (OP2Shell 0x13006D6C, 0x13006DB7) |
| Auto demo | −2 | `a` | not listed: OP2Shell names `ademo1.dll` to `ademo3.dll` itself |
| Tutorial | −3 | `t` | Tutorial dialog: `t*.dll`, type must be −3 (OP2Shell 0x13006DA5, 0x13006DDC) |
| Land Rush | −4 | `mu`*N* | multiplayer lobby, below |
| Space Race | −5 | `mf`*N* | |
| Resource Race | −6 | `mr`*N* | |
| Midas | −7 | `mm`*N* | |
| Last One Standing | −8 | `ml`*N* | |

*N* is the player count, one digit. The values are the ones [opm.md](../docs-community/opm.md#missiontype) lists for the editor.

The game itself tests the type in a few places: multiplayer is a type from −8 to −4 (0x4028E0, also 0x403385),
a campaign is a type above 0 (0x402930), a colony game exactly −1 (0x402960).

### Colony games and tutorials (OP2Shell.dll)

One list builder (OP2Shell 0x13006860) fills both dialogs. It searches two folders, the game's own (the folder of
`Outpost2.exe`, from `GetModuleFileNameA`) and the folder where the game finds `I.AVI` (the CD on a CD install),
for the dialog's pattern. For each file not already listed it calls `GetModuleDesc` (below) on the full path; when
that succeeds and the type equals the dialog's (−1 or −3), the `LevelDesc` text is added to the list and the file
name kept as the item's data. A DLL whose name matches but whose type differs is skipped, so the prefix alone
doesn't make a colony game. `us_them.dll` (type −1, 4 players, "This tests building all the buildings and units.")
is in no stock list because its name doesn't start with `c`.

### Multiplayer (Outpost2.exe)

The lobby's list builder (0x497780) searches the same two folders for `*.dll`. A file is listed when all of these
hold:

1. Its name parses (0x4029C0): at least 3 characters; the first is `m` or `M`; the second is `u`, `f`, `r`, `m` or
   `l` in either case, giving the type −4, −5, −6, −7 or −8; the third is a digit from 2 to 6, the player count.
2. That type lies in the range the lobby asks for, and the name's player count is at least the lobby's.
3. It isn't already listed (same name from the other folder).
4. `GetModuleDesc` succeeds, `DescBlock`'s player count is at least the lobby's, and `DescBlock`'s type also lies in
   the range.

The name and `DescBlock` are checked separately, so they needn't agree exactly. The `LevelDesc` text goes into the
list and the file name into the item's data (0x497AA5, 0x497B12).

### Under OPU

OPU replaces both list builders: `OPUPatch.dll` registers patches on 0x497780 and on OP2Shell's 0x13006860 (its
patch descriptors at 0x16012F99 and 0x16013002), so OPU's lists also show the missions under `OPU\maps`. OPU's own
change notes say they appear in the Colony Games and multiplayer dialogs. **Unconfirmed:** the rules OPU's
replacements apply. Leads: OP2Lua's notes say the `c` prefix is still what puts a DLL in the colony list, and that a
multiplayer mission is listed under its `DescBlock` type, though its `TugOWarLua.dll` and `NostalgiaLua.dll` have
names that don't start with `m`; the replacement functions in `OPUPatch.dll` (from 0x16011190) would settle it.

Names measured in the 159: every Sierra DLL follows the table. Of the 53 under `OPU\maps`, the colony games all start
with `c` (18), and the multiplayer ones start with `m` except `TugOWarLua.dll` and `NostalgiaLua.dll`; `ml_demo.dll`
has `_` where the digit goes, and `mr4Yuko.dll` says 4 players in its name and 5 in its `DescBlock`.

## Exports

The game looks these names up. All are case-sensitive.

| Export | Kind | Looked up by | Required |
|---|---|---|---|
| `DescBlock` | data, 16 bytes used | `GetModuleDesc` (0x402619), `LoadScript` (0x402BEF) | yes, for both |
| `LevelDesc` | data, NUL-terminated text | `GetModuleDesc` (0x40263D) | yes, to be listed or started |
| `MapName` | data, NUL-terminated text | `GetModuleDesc` (0x402629) | yes, to be listed or started |
| `TechtreeName` | data, NUL-terminated text | `GetModuleDesc` (0x402653) | yes, to be listed or started |
| `DescBlockEx` | data, 4 bytes used | `GetModuleDescEx` (0x4027E5) | no |
| `InitProc` | function | `LoadScript` (0x402BA4) | no |
| `AIProc` | function | `LoadScript` (0x402BB2) | no |
| `StatusProc` | function | `LoadScript` (0x402BBF) | no |
| `GetSaveRegions` | function | `LoadScript` (0x402BCC) | no |
| trigger functions | function, any name | the trigger factories, by the name the mission passes (0x4757D0) | each one named |
| data references | data, any name | the same lookup (0x4035D0), e.g. the unit block that `CreateUnitBlock` names (op2-rs) | each one named |

The data exports are read from the file (`GetModuleDesc`) and must be initialised data. No DLL and no part of the
game uses an export called `DLLDesc`: the name isn't in `Outpost2.exe`, `OP2Shell.dll`, OPU's DLLs or any mission DLL.

Measured in the 159 mission DLLs: every one exports `DescBlock`, `LevelDesc`, `MapName`, `TechtreeName`, `InitProc`
and `AIProc`. `StatusProc` is in 105 of the 106 Sierra DLLs (not `us_them.dll`) and in 24 of the 53 others.
`GetSaveRegions` is in 62 Sierra DLLs (the campaigns, tutorials, five colony games and 19 multiplayer games; not the
demos, `us_them.dll` or the other 40 multiplayer games) and in 42 others. `DescBlockEx` is in 2 Sierra DLLs
(`ml2_21`, `ml4_21`) and 28 others. A DLL has 8 to 122 exports (Sierra: 8 to 72); 3433 in all, 2377 in `.text`,
1018 in `.data`, 38 in `.rdata`. Besides the names above there are 1716 function exports (trigger callbacks such as
`NoResponseToTrigger`, which 134 DLLs export) and 390 data exports in 35 DLLs (unit and group tables with names like
`AIInitialBase`, `Group1`, `Units`).

### `DescBlock`

| Offset | Type | Field | Used |
|---|---|---|---|
| 0 | i32 | mission type (table above) | yes |
| 4 | i32 | number of players | yes |
| 8 | i32 | maximum tech level | yes |
| 12 | i32 | unit-only mission: 1 or 0 | yes |
| 16 | u32 × 3 | pointers to `MapName`, `LevelDesc`, `TechtreeName` | no: replaced |
| 28 | u32 | checksum, 0 | no: replaced |

`GetModuleDesc` copies 28 bytes from the export (0x4026B3) and then overwrites offsets 16 to 31 with its own copies
of the three strings and the file's checksum, so only the first 16 bytes matter. The Sierra DLLs and 12 of the
others carry the full 32-byte form, with the three pointers in that order and a last word of 0 (100 Sierra DLLs) or
a stale value (6). The other 41 have a 16-byte `DescBlock` followed by unrelated data, which works the same.

- **Players.** For a campaign or colony game the number of player slots the mission uses, AI included (1 to 4 in
  the shipped DLLs). For a multiplayer game the human players; the lobby lists the mission only for that many
  players or fewer.
- **Maximum tech level.** Passed to the tech tree loader with `TechtreeName` (0x4894D2): topics above it aren't
  available (see [tech-trees.md](tech-trees.md)). The campaigns use their mission number, t13 and t14 use 11, the
  demos 8, the rest 12; OPU's Exiles missions use 3, 4, 5, 10 and 10.
- **Unit-only mission.** The colony report and economy update is skipped for human players (0x489F30): food, ore,
  colonists and morale stay as `InitProc` set them (measured in op2-rs on e06). Set in e04, e06, e09, e11, p04, p06,
  p09, p10 and t02, and in five OPU missions (`mm4_x02`, `mm4_x05`, `cResEsc`, `mu2Resc`, `ml6_sr`).

### `DescBlockEx`

32 bytes in every DLL that has it; the game reads only the first i32, the number of extra AI players in a
multiplayer game. At the start of a multiplayer game (start flag 0x10) the player count is the lobby's plus this
value (0x48958F..0x4895B9). Values measured: 1 in 15 DLLs (both Sierra ones), 2 in 4, 0 in the 11 OP2Lua missions.

### The strings

`LevelDesc` is the text the menus show (up to 48 characters in the shipped DLLs, e.g. "Eden Mission #1",
"2 Player, LastOne, 'Great Divide' map."). `MapName` and `TechtreeName` are bare file names, no folder: the game
finds them by name in its search path (see [The map and the tech tree](#the-map-and-the-tech-tree)). Case varies
(`EDEN01.map`, `PLY12.map`, `MULTITEK.TXT`, `survtech.txt`) and doesn't matter. In the 159 DLLs, `TechtreeName` is
`MULTITEK.TXT` in 90, `PLY_TEK.TXT` 16, `EDENTEK.TXT` 13, `TUTORTEK.TXT` 12, and a pack's own file in the other 28.

### The functions

All are `__cdecl`.

| Export | Signature | Called |
|---|---|---|
| `InitProc` | `int InitProc(void)` | once, when a new game starts, after the map, tech tree and DLL are loaded (0x402F60). Returning 0 fails the start |
| `AIProc` | `void AIProc(void)` | every 4 ticks, from the mission cycle (0x403309) |
| `GetSaveRegions` | `void GetSaveRegions(SaveRegion *out)` | once each time the DLL is loaded (0x402BE1), before `InitProc` |
| `StatusProc` | `int StatusProc(void)` | **Unconfirmed**: the game stores its address (MissionManager +0xC, 0x402BC4) but no code was found that reads it back (no reference to its slot 0x4EFD24, and the manager's methods 0x402B10-0x4036C0 don't use the field) |
| trigger functions | `void F(void)`, or `void F(OnTriggerArgs *)` under OPU | when the trigger fires, in the mission cycle |

The mission cycle (0x403230) runs on ticks where `tick & 3 == 3`, before the units move: the triggers that fired
have their functions called, then the groups are updated, then `AIProc`, then the victory and failure checks
(measured in op2-rs). A missing `InitProc` or `AIProc` is skipped (0x402F5C, 0x403305).

## How the game reads a DLL

### The description, without loading the DLL

`GetModuleDesc` (0x4025B0, exported by `Outpost2.exe` for OP2Shell as `?GetModuleDesc@@YIPAUAIModDesc@@PBD@Z`,
fastcall, file name in ECX) maps the file into memory and reads its exports with its own PE reader (0x419480); no
DLL code runs. It needs all four of `DescBlock`, `MapName`, `LevelDesc` and `TechtreeName`, or it returns null. It
returns a 32-byte structure that `FreeModuleDesc` (0x402890) frees:

| Offset | Field |
|---|---|
| 0 | `DescBlock`'s four i32 |
| 16 | copy of `MapName` |
| 20 | copy of `LevelDesc` |
| 24 | copy of `TechtreeName` |
| 28 | checksum of the whole file (below) |

The PE reader is stricter than Windows':

- It wants `MZ` and `PE\0\0`, at least one data directory and at most 128 sections, and it takes the section table
  to start 0xF8 bytes after the `PE` signature, so the optional header must be the standard 224 bytes.
- It finds the section that holds the export directory by `VirtualAddress` to `VirtualAddress + SizeOfRawData` and
  converts addresses to file offsets with that section's `PointerToRawData`; an exported item is found the same way
  and must lie inside the file.
- At most 0x8000 names. The names are searched one by one with a byte comparison.
- The **name's index** picks the entry of the address table: the ordinal table is never read (0x4195F9..0x419605). A
  DLL whose name *i* doesn't belong to address *i* gets the wrong export.

In all 159 mission DLLs the ordinals are the identity (name *i* is ordinal *i*, base 1), the names are sorted, the
optional header is 224 bytes and every `DescBlock` is in `.data` (150) or `.rdata` (9), at least 132 bytes before the
end of its section's file data. A normal MSVC or MinGW build without a `.def` file that assigns ordinals gives this.

The checksum (0x40C050) starts at 0; the first `size mod 4` bytes are shifted in one at a time (`eax = (eax | b)
<< 8`) and the result rotated right by 1; then each following u32 is XORed in and the result rotated right by 1. A
result of 0 becomes 1. The size test uses a 16-bit `jcxz`, so a file whose size is a multiple of 65536 bytes gets 1
(none of the 159 is). The multiplayer start keeps this checksum and the map's (0x450054, 0x450063).
`GetModuleDescEx` (0x402780) reads `DescBlockEx` the same way and returns its first i32.

### Starting a game

From the start info's DLL name (0x489425 onwards):

1. The name is looked up in the game's search path (ResManager, 0x471590) and `GetModuleDesc` reads the file.
2. The tech tree `TechtreeName` is loaded with the maximum tech level (0x4894E3), then the map `MapName`
   (0x4894FD).
3. `LoadScript` (0x402B20) calls `LoadLibraryA` on the full path. On failure it shows "Failed to load script %s,
   error %i." with `GetLastError`. It resolves `InitProc`, `AIProc`, `StatusProc` and `GetSaveRegions` with
   `GetProcAddress`, calls `GetSaveRegions` if present (else the region pointer is set to 0), and resolves
   `DescBlock`; without `DescBlock` the DLL is freed and the start fails. It keeps the path.
4. In a multiplayer game, `DescBlockEx` adds AI players (above).
5. `InitLevel` (0x402F20) sets up each of the six player slots' default group (0x402F80 for a slot whose
   per-player word at 0x56EF44 + 0xC24 × *n* is 0, −1 for the others), then calls `InitProc`; 0 from `InitProc`
   fails the start (0x48987F).

At the end of a game `Deinit` (0x402C20) calls `FreeLibrary`. Loading a saved game loads the DLL again (below) and
doesn't call `InitProc`.

## The save region

`GetSaveRegions` fills an 8-byte structure:

| Offset | Type | Field |
|---|---|---|
| 0 | pointer | start of the data to save, or 0 for none |
| 4 | u32 | its size in bytes |

The game asks once per load of the DLL and keeps the answer, so the pointer must stay valid for as long as the DLL is
loaded: in practice a global in the DLL's data section. Saving (0x402C60) and loading (0x402DC0) the mission block of
a saved game ([saved-games.md](saved-games.md#mission-block)):

| Field | Written when |
|---|---|
| u32 0xAEDE0034 | always |
| u32 length, then the DLL's file name and extension (no folder; from `_splitpath`) | always |
| u32 size | the region pointer isn't 0 |
| *size* bytes from the pointer | the pointer isn't 0 and the size isn't 0 |
| 0x30 bytes: the six players' default group records | always |
| the script objects (triggers, groups, ...) | always |
| u32 0xAEDE0034 | always |

So a DLL without `GetSaveRegions`, or one that reports a null pointer, writes no size at all; a non-null pointer with
size 0 writes the size word only. On loading, the game finds the DLL by its name in the search path and runs
`LoadScript` again, which calls `GetSaveRegions` on the freshly loaded DLL. If that returns a pointer, the stored size
must equal the size it reports now, or the load fails (0x402E9D); then the bytes are copied over the region.

The bytes are copied raw. Everything else in the DLL starts over from the file's values when a save is loaded
(measured in op2-rs on c_euf.dll), so a mission must keep all state that matters in its region, and pointers saved
inside it point into the old process. Save regions measured from the code of each `GetSaveRegions` (all are a few
instructions storing constants): 1 byte in the 14 tutorials, 0x14 to 0xBC bytes in the campaigns, 0x24 to 0xBC in the
colony games, 4 to 0x2C in the multiplayer games that have one. Of the 42 OPU missions with the export, 14 report a
region (1 to 0x4D0 bytes, the largest `c_dcsmp.dll`'s), 16 a null pointer, the 11 OP2Lua missions a null pointer
through their runtime, and `ml4_cft.dll` jumps into other code (not followed).

## The map and the tech tree

A DLL doesn't open its own map or tech tree: the game reads `MapName` and `TechtreeName` from the file before the
DLL is loaded and opens them by name, through the same search the game uses for every data file (loose files in its
folders, then the `.vol` archives; see [volumes.md](volumes.md)). `MapName` is a [`.map`](maps.md) file;
`TechtreeName` a [tech tree](tech-trees.md). Several missions can share one map or tree.

Under OPU the search starts with the mission's own folder. OPU's change notes describe the order: a map's named
folder (`OPU\base\maps\<map>` or `OPU\maps\<pack>`) first while that map is being loaded, then the `OPU` folder,
then the game folder, the `.vol` files and the CD. Measured in op2-rs: where several packs ship a `survtech.txt`
with different contents, each mission's game loaded the one in its own folder. **Unconfirmed:** how OPU picks between
two packs that hold the same file name when neither is the mission's folder.

## OPU's folders

OPU's installer copies each Sierra mission's DLLs and maps into `OPU\base\maps\<name>` (measured in op2-rs), so 40
folders: one per multiplayer map (`AlliedSiege`, `TugOWar`, ...), one per colony game, and `campaign`, `tutorial`
and `misc` (the demos and `us_them.dll`). Community missions sit in `OPU\maps\<pack>`: 31 packs here, each a folder
holding one or more mission DLLs and whatever they need that the game doesn't have.

| File in a pack | Count | What |
|---|---|---|
| `*.dll` with `DescBlock` | 53 | the missions |
| `op2extra.dll` | 3 | a helper library (the same file in three packs) that their missions import |
| `*.map` | 27 | the pack's maps; other missions name a stock map |
| `*.txt` | 26 | tech trees (`survtech.txt`, `cooptech.txt`, ...), Exiles' five briefings, OP2Lua's readme |
| `*.lua` | 22 | OP2Lua missions ([lua-missions.md](../docs-community/lua-missions.md)) |

OPU's change notes also say DLLs may be placed anywhere in this folder tree or in `OPU\libs`, which is how a mission
finds a library such as `op2extra.dll` or `OP2LuaCore.dll`.

### OPU's extra exports

OPU 1.4.1 calls six more exports when the mission DLL has them. `OPUPatch.dll` looks them up with `GetProcAddress`
(0x16003DA0..0x16003DFE), and OPU's change notes give their meaning:

| Export | Signature | When |
|---|---|---|
| `OnLoad` | `int OnLoad(OnLoadArgs *)` | when the DLL is loaded; 1 = success |
| `OnUnload` | `int OnUnload(OnUnloadArgs *)` | when the DLL is unloaded, also when a save is loaded |
| `OnEnd` | `void OnEnd(OnEndArgs *)` | at the end of a mission, with its results |
| `OnChat` | `void OnChat(OnChatArgs *)` | when any player sends a chat message |
| `OnCreateUnit` | `void OnCreateUnit(OnCreateUnitArgs *)` | when a unit is created |
| `OnDestroyUnit` | `void OnDestroyUnit(OnDestroyUnitArgs *)` | when a unit is destroyed |

Each argument structure starts with its own size. Newer mission headers (TethysAPI) name the first three
`OnLoadMission`, `OnUnloadMission` and `OnEndMission` and add `OnSaveGame`, `OnLoadSavedGame`, `OnDamageUnit`,
`OnTransferUnit` and `OnGameCommand`; none of those names is in `OPUPatch.dll` 1.4.1, so OPU 1.4.1 doesn't call them.
The 11 OP2Lua DLLs export the newer names; no other mission DLL here exports an `On` callback.

## Measured values

The 106 Sierra DLLs: linker 4.20 (MSVC 4.2), image base 0x11000000, 20992 to 107520 bytes, all importing from
`Outpost2.exe`. The 53 under `OPU\maps`: linkers 4.20 (9), 6.00 (4), 7.10 (5), 8.00 (2), 9.00 (3), 14.16 (14), 14.28
(5), 14.44 (2) and 14.51 (9); image base 0x11000000 (32) or 0x10000000 (21); 8704 to 1382400 bytes. All but the 11
OP2Lua DLLs import from `Outpost2.exe` (18 to 168 functions).

| Type | Sierra DLLs | Players | OPU missions |
|---|---|---|---|
| campaign 1-12 | 24 (`e01`-`e12`, `p01`-`p12`) | 1 to 3 | 0 |
| colony −1 | 6 (`CEP1`, `CES1`, `CPP1`, `CPS1`, `CPS2`, `us_them`) | 2 (`us_them` 4) | 18 |
| auto demo −2 | 3 | 2 or 3 | 0 |
| tutorial −3 | 14 | 2 | 0 |
| Land Rush −4 | 13 | name's digit | 3 |
| Space Race −5 | 8 | | 9 |
| Resource Race −6 | 7 | | 1 |
| Midas −7 | 6 | | 5 |
| Last One Standing −8 | 25 | | 17 |

## Making one

- A 32-bit PE DLL with the standard 224-byte optional header and identity export ordinals (the default for MSVC and
  MinGW without a `.def` file that sets ordinals).
- Export, as initialised data, `LevelDesc`, `MapName` and `TechtreeName` (NUL-terminated) and `DescBlock` (four
  i32: type, players, maximum tech level, unit-only flag). Pad `DescBlock` to 32 bytes to match the Sierra layout;
  the game doesn't need it.
- Name it by the prefix rules if it should appear in the stock menus; under OPU put it in its own folder under
  `OPU\maps` with its map and tech tree when they aren't stock files.
- Export `InitProc` to set up the level and `AIProc` for anything that runs during play; export each function named
  in a trigger.
- To survive save and load, keep the mission's state in one static block and export `GetSaveRegions` reporting it;
  changing that block's size breaks older saves.
- A multiplayer mission with computer players exports `DescBlockEx` with the AI count first.

## Sources

- Disassembly of GOG `Outpost2.exe` 1.2.0.7: `GetModuleDesc` 0x4025B0 and its PE reader 0x419480, `GetModuleDescEx`
  0x402780, the file checksum 0x40C050, `LoadScript` 0x402B20, `Deinit` 0x402C20, save 0x402C60 and load 0x402DC0,
  `InitLevel` 0x402F20, the mission cycle 0x403230, the export resolver 0x4035D0, the multiplayer name parser 0x4029C0
  and list builder 0x497780, the game start 0x489425..0x4895B9, the campaign name formats 0x48940B and 0x48772D.
- Disassembly of `OP2Shell.dll`: the colony and tutorial list builder 0x13006860 and its two set-ups. Of OPU 1.4.1's
  `OPUPatch.dll`: the extra export lookup 0x16003DA0 and the patch descriptors 0x16012F99, 0x16013002.
- The mission SDK headers, as leads checked against the files and the exe: Outpost2DLL's `RequiredExports.h` (the
  OP2DotNetMissionSDK's native part) and TethysAPI's `API/Mission.h` and `Game/MissionManager.h` (OutpostUniverse).
- OPU 1.4.1's `CHANGES.md`: the folder layout and search order, the extra exports.
- op2-rs (bufke's reimplementation): measured the mission cycle's order and timing, the unit-only flag's effect, the
  reload of a DLL's globals after a load, the tech tree beside the DLL, and OPU's installer layout.
- Measured: the 284 DLLs in the game folder and under `OPU`, with a probe reading PE headers, export tables and
  imports, and decoding each `GetSaveRegions`; the 106 game-folder copies compared with `OPU\base\maps`.
