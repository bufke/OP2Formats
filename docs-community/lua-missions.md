# Lua missions (`.lua` + `.placement.lua`)

The `.lua` missions in `OPU\maps\LUA` and `OPU\maps\Nostalgia` are not an Outpost 2 or OPU format. They belong to
**OP2Lua**, Leviathan's own project (MIT licence, "Outpost Universe" in its copyright line), a runtime that runs
missions written in Lua on top of the ordinary [mission DLL](../docs/mission-dlls.md) interface. It is not part of OPU 1.4.1:
OPU's readme and change notes don't mention it, and OPU's own DLLs don't load Lua. The game sees only a small mission
DLL per mission; that DLL hands everything to OP2Lua's runtime, which reads the two Lua files.

The facts here come from OP2Lua's source (version 0.7.0: the stub `stub/LuaMission.cpp`, the runtime's
`src/Mission.cpp`, `Placement.cpp`, `LuaHost.cpp`, `Names.cpp`, and the scaffolder `op2lua-newmission`) and from
the installed files, measured with a PE probe: 11 missions, each a DLL, a `<Name>.lua` and a `<Name>.placement.lua`.

## The files

| File | Where | What |
|---|---|---|
| `OP2LuaCore.dll` | `OPU\` | the runtime: Lua 5.4.7, the game bindings, the placement loader, the scheduler. One copy for all missions |
| `<Dll>.dll` | the mission's folder | the stub the game lists and loads: 109056 or 109568 bytes, the same code in every mission |
| `<Name>.lua` | next to the stub | the mission's logic |
| `<Name>.placement.lua` | next to the stub | the starting layout: players, units, beacons, walls, regions, markers |

Installed here: nine missions in `OPU\maps\LUA` (`cHoldtheBeacon`, `cHoldtheLine`, `cLaCorridaAITest`,
`cLaCorridaSimpleAITest`, `cPeacekeeperAITest`, `cPeacekeeperSimpleAITest`, `cSeekandDestroy`, `cStrikeTeam`,
`cTheConvoy`, all colony games), `TugOWarLua` in the same folder and `NostalgiaLua` in `OPU\maps\Nostalgia` (both
multiplayer Last One Standing). `OPU\maps\LUA\README.txt` describes the five demo missions; `OPU\tools\OP2LuaEditor.exe`
is OP2Lua's editor.

The game finds `OP2LuaCore.dll` because OPU lets DLLs sit in its folder tree (OPU's change notes); the stub imports it
by name and nothing else besides `KERNEL32.dll`, so unlike other mission DLLs it imports nothing from `Outpost2.exe`.

## The stub DLL

A normal mission DLL as [mission-dlls.md](../docs/mission-dlls.md) describes, with fixed-size data exports so that a tool can
patch them without a compiler:

| Export | Contents |
|---|---|
| `LevelDesc` | `char[1024]`, the menu name |
| `MapName` | `char[1024]`, the map the game loads |
| `TechtreeName` | `char[1024]`, `MULTITEK.TXT` in all 11 |
| `DescBlock` | 16 bytes: type, players, maximum tech level 12, unit-only 0 |
| `DescBlockEx` | 32 bytes of 0: no multiplayer AI |
| `InitProc`, `AIProc`, `GetSaveRegions` | forwarded to the runtime |
| `OnLoadMission`, `OnUnloadMission`, `OnEndMission`, `OnSaveGame`, `OnLoadSavedGame`, `OnChat`, `OnCreateUnit`, `OnDestroyUnit`, `OnDamageUnit`, `OnTrigger` | forwarded to the runtime |
| `OP2LuaTrigger` | an empty function, the callback the runtime names in its own victory and failure triggers |

The three strings sit at 0x400-byte steps in `.data` (RVA 0x19000, 0x19400, 0x19800 in `cStrikeTeam.dll`), then
`DescBlock`. The 11 stubs' `DescBlock` values: type −1 and 2 players for the nine colony games, −8 with 2
(`TugOWarLua`) or 4 (`NostalgiaLua`) players.

A new mission is the stub copied and patched (`op2lua-newmission`, `new-mission.ps1`, the editor): the template's
`LevelDesc` text `OP2Lua mission (unpatched template)` and `MapName` text `eden01.map` are found by searching the file
and overwritten (the new text, at most 1022 characters, and NULs over the rest of the old text); for another type or player count the 16 bytes
`FF FF FF FF 02 00 00 00 0C 00 00 00 00 00 00 00` are found and their first two i32 rewritten; the version resource's
file description is set to the name. The patched file is named `<Base>.dll`, base being the mission name without
characters other than letters and digits.

## How a mission is loaded

1. The game lists the stub by its `DescBlock` and `LevelDesc`, like any mission DLL. A leading `c` on the file
   name is what puts a colony game in the Colony Games list (OP2Lua's notes; see [mission-dlls.md](../docs/mission-dlls.md#under-opu)).
2. The game loads the map named in `MapName` and the tech tree, then the stub, and calls `InitProc`.
3. On its first call (`InitProc`, or `OnLoadMission`), the stub takes its own path from `GetModuleFileNameW`: the
   folder, and the file name without `.dll` as the **base name**. It passes both to the runtime.
4. The runtime makes a fresh Lua state and runs the placement file, then the mission file, in that state. Each is
   the first that exists of:

   | Placement | Mission |
   |---|---|
   | `<folder>\<base>.placement.lua` | `<folder>\<base>.lua` |
   | `<folder>\<alt>.placement.lua` | `<folder>\<alt>.lua` |
   | `<folder>\placement.lua` | `<folder>\mission.lua` |

   *alt* is the base name without its first letter when that is `c` or `C` followed by an upper-case letter, so
   `cStrikeTeam.dll` uses `StrikeTeam.lua` and `StrikeTeam.placement.lua`. All 11 missions here use the unprefixed
   names. Without a placement file the mission runs script-only.
5. Still inside `InitProc`, the runtime applies the placement (below), then calls the mission's `on_init`; its
   result is `InitProc`'s. Returning `false` or `nil` fails the start; returning nothing, any other value, a missing
`on_init` or an error in it all count as success.

The map is the DLL's `MapName`, never the placement's `map` field: the game has loaded the map before any Lua runs.
When the two differ, the runtime writes a warning to its log.

## The mission file

An ordinary Lua chunk, run once at load. It defines global functions that the runtime calls when the game calls the
matching DLL export; each is optional and runs in a protected call (an error is logged, never passed to the game).

| Function | Arguments | Called from |
|---|---|---|
| `on_init` | | `InitProc`, after the placement |
| `on_tick` | | `AIProc`, every 4 ticks, after the scheduler |
| `on_chat` | the chat message | `OnChat` |
| `on_create_unit` | the unit | `OnCreateUnit` |
| `on_destroy_unit` | the unit | `OnDestroyUnit` |
| `on_damage_unit` | target, source, damage | `OnDamageUnit` |
| `on_load` | | `OnLoadMission` |
| `on_unload` | | `OnUnloadMission` |
| `on_end` | | `OnEndMission` |
| `on_save` | | `OnSaveGame` |
| `on_load_save` | | `OnLoadSavedGame` |

OPU 1.4.1 calls its extra exports by the names `OnLoad`, `OnUnload`, `OnEnd`, `OnChat`, `OnCreateUnit` and
`OnDestroyUnit` (see [mission-dlls.md](../docs/mission-dlls.md#opus-extra-exports)). So under OPU 1.4.1 `on_chat`,
`on_create_unit` and `on_destroy_unit` can run, and `on_load`, `on_unload`, `on_end`, `on_save`, `on_load_save` and
`on_damage_unit` are never called. **Unconfirmed** in the running game; a mission that logs from `on_end` would show
it.

All 11 missions define `on_init`; `HoldtheLine.lua` also defines `on_destroy_unit`. No other callback is used.

Before the mission file runs, the runtime defines these globals (besides its `game`, `mission` and other API tables,
not covered here):

| Name | What |
|---|---|
| `after(ticks, fn)` | run `fn` once, `ticks` from now |
| `every(ticks, fn)` | run `fn` every `ticks`; returns a handle with `cancel()` |
| `when(pred, fn)` | run `fn` once, the first time `pred()` is true |
| `at(tick, fn)`, `at_mark(m, fn)` | run `fn` at an absolute tick, or mark (100 ticks) |
| `marks(n)` | `n × 100` ticks |
| `print`, `log.mission`, `log.ai`, `log.show_ai_in_game`, `log.ai_status` | logging |
| `players`, `units`, `regions`, `markers` | the handles the placement made (below) |

The scheduler runs from `AIProc`, so timers are checked every 4 ticks. The Lua state opens only the `base`,
`string`, `table` and `math` libraries: no `io`, `os` or `package`.

## The placement file

A Lua chunk that **returns one table**; anything else is refused with a log message. The runtime reads these keys
and ignores the rest. Missing keys mean nothing is placed. Coordinates are the ones the game's status bar shows:
file tile + 1 on both axes (the runtime adds the map's 32-column padding on maps that don't wrap; see
[maps.md](../docs/maps.md#coordinates)).

| Key | Value |
|---|---|
| `players` | table indexed 1-6: one player each (below). Leave it out in a multiplayer mission, where the lobby has set the players |
| `units` | list of units |
| `beacons` | list of mining beacons, fumaroles and magma vents |
| `walls` | list of walls and tubes |
| `regions` | table name → `{x, y}` (a tile) or `{x1, y1, x2, y2}` (a rectangle, either corner order) |
| `markers` | table name → `{at = {x, y}, type = "Circle"}` (`DNA`, `Beaker`) |
| `map` | text, compared with the DLL's `MapName` only for the warning |

The shipped files also carry `name`, `tech`, `type`, `max_tech` and `players_count`, which the runtime doesn't read
(the stub's `DescBlock` holds the real values), and some players carry `bot` and `morale`, likewise unread.

**A player** (key: the player number, 1-based):

| Field | Default | Value |
|---|---|---|
| `colony` | `"Eden"` | `"Eden"` or `"Plymouth"` |
| `human` | `true` | human or computer |
| `color` | unchanged | `Blue`, `Red`, `Green`, `Yellow`, `Cyan`, `Magenta`, `Black`, `White` (0-7) |
| `resources` | unchanged | `common_ore`, `rare_ore`, `food`, `kids`, `workers`, `scientists` (each set, 0 if left out), `solar_satellites` and `tech_level` (set when above 0), `completed_research` (a list of topic ids) |
| `center_view` | none | `{x, y}` for the view at the start |
| `allies` | none | list of player numbers, allied both ways |

**A unit**:

| Field | Default | Value |
|---|---|---|
| `type` | required | a unit type name: `CargoTruck`, `ConVec`, `Lynx`, `CommandCenter`, `Tokamak`, ... (70 names, those of the game's unit types, the weapons and `Wreckage`); an unknown name skips the entry |
| `player` | 1 | owner |
| `at` | `{0, 0}` | `{x, y}` |
| `weapon` | none | a weapon type name, for combat vehicles and guard posts |
| `direction` | `"East"` | `East`, `SouthEast`, `South`, `SouthWest`, `West`, `NorthWest`, `North`, `NorthEast` |
| `lights` | `true` | headlights on |
| `health` | 1.0 | fraction of full hit points |
| `cargo`, `amount` | none, 1000 | a Cargo Truck's load: `Food`, `CommonOre`, `RareOre`, `CommonMetal`, `RareMetal`, `CommonRubble`, `RareRubble`, `Spacecraft`, `Wreckage`, `GeneBank` (`Metal`, `Ore`, `Rubble` are aliases) |
| `kit`, `kit_topping` | none | a ConVec's structure kit, and the kit's weapon |
| `name` | none | publishes the unit as `units[name]` |

**A beacon**: `type` (`MiningBeacon` by default, `Fumarole`, `MagmaVent`), `ore` (`Common`, `Rare`, else random),
`yield` (`Bar1`, `Bar2`, `Bar3`, else random), `variant` (`Variant1`-`Variant3` or `High`, `Mid`, `Low`, else random),
`at`, and an optional `name` (published in `units`).

**A wall**: `type` (`Wall` by default, `LavaWall`, `MicrobeWall`, `Tube`) and `at`.

The runtime applies the table in this order: players, then a `players[n]` handle for every player in the game
(listed or not), beacons (before units, so a mine finds its beacon), units, walls, regions, markers. Then it
publishes `players`, `units`, `regions` and `markers`. `markers[name]` holds the game's internal coordinates
(`x`, `y`), not status-bar ones.

Measured in the 11 placement files (33 to 84 lines): all return a table with `beacons`; nine have `players` (both
multiplayer missions leave it out); eight have `walls`; the five demo missions alone have `units`, `regions` and
`markers`; `TugOWarLua` places only tubes and beacons, `NostalgiaLua` only its 26 beacons.

## Saved games

The stub's `GetSaveRegions` reports no region, and the Lua state is never saved. Loading a saved game loads the DLL
again without calling `InitProc` (see [mission-dlls.md](../docs/mission-dlls.md#starting-a-game)), and under OPU 1.4.1
`OnLoadMission` isn't called either, so the runtime would never load the scripts. **Unconfirmed**: that a loaded
OP2Lua game then runs without its script; loading a save of one would settle it.

## Logs

The runtime writes `logs\OP2Lua.log` (its own record, kept across missions), `logs\OP2Lua-<base>.log` (the mission's
`print` output and errors), `logs\OP2Lua-<base>-AI.log` and `logs\OP2Lua-<base>-AI-status.json`, relative to the
game's working directory. OP2Lua's readme places them in `OPU\logs`. **Unconfirmed**: that the working directory is
the `OPU` folder (the runtime's source says so in a comment; the folder is empty in this install).

## Making one

- Copy and patch the stub (OP2Lua's tools do this), or build a mission DLL that forwards to `OP2LuaCore.dll` the same
  way. Name it `c<Name>.dll` for a colony game.
- Write `<Name>.placement.lua` returning the table above and `<Name>.lua` defining `on_init`; put the three files in
  one folder under `OPU\maps`.
- Use status-bar coordinates in the placement, and the map the DLL names.
- For a multiplayer mission leave `players` out and build what depends on the lobby in `on_init`.

## Sources

- OP2Lua 0.7.0 (Leviathan): `stub/LuaMission.cpp` (the exports, the base name), `src/Mission.cpp` (the file lookup,
  the callbacks, the map check), `src/Placement.cpp` (the placement keys, defaults and order), `src/LuaHost.cpp` (the
  libraries and the scheduler), `src/Names.cpp` (cargo names), `op2lua-newmission/main.cpp` (patching the stub), and
  its docs `ARCHITECTURE.md`, `API.md` and `CHANGES.md`.
- TethysAPI's `API/GameMap.h` (OutpostUniverse): the status-bar coordinate conversion the runtime uses.
- OPU 1.4.1's `README.md`, `CHANGES.md` and `OPUPatch.dll` (the names of the extra exports it calls).
- Measured: the 11 installed stub DLLs (exports, `DescBlock`, sizes, imports) with a PE probe; the 22 `.lua` files
  (top-level keys and the callbacks each defines).
