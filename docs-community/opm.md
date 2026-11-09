# Missions (`.opm`)

An `.opm` file is a mission written as JSON: the level settings, each player's starting resources, units, walls and
tubes, the beacons, markers and wreckage on the map, the music, and optional disasters, regions and triggers.
OP2MissionEditor saves one next to the mission's `.map` and the mission DLL it builds; missions made with the
OP2DotNetMissionSDK read it at run time through the SDK's mission reader. The game itself never reads an `.opm`.
Read and written by op2-opm.

Facts here come from op2-opm's model, the OP2DotNetMissionSDK's source (`MissionReader/Json`, which defines the
member names), and these files: two missions saved with OP2MissionEditor (op2-opm's `tests/data`: `opu_12.opm`,
20514 bytes, and `cmyTut.opm`, 9801 bytes) and three SDK test missions (`cTest.opm`, `cTest-6p.opm`,
`cTest-bak.opm`). Where only the SDK's source says something, the text says so.

## Writers and layouts

The same members come in two layouts:

| | OP2MissionEditor | SDK 3.0 tools, hand-edited files |
|---|---|---|
| Serializer | .NET `DataContractJsonSerializer`, then the editor's own formatter | Newtonsoft.Json |
| Indent | tabs | two spaces |
| Member order | alphabetical | declaration order |
| Default values | every member written | defaults often left out |

A reader must find members by name, never by position, and give an absent member its default. The editor's layout
is described byte for byte in [The editor's text layout](#the-editors-text-layout).

The SDK itself reads and writes the file with .NET's `DataContractJsonSerializer` (`MissionReader.GetMissionData`
and `WriteMissionData`), which ignores layout on reading and writes members in name order with no white space. No
SDK source on disk uses Newtonsoft.Json. **Unconfirmed:** the two-space column. It comes from OPM_FORMAT.md (its
samples `cSDKFloodPlain.opm` and `cSDKPieChart.opm`) and op2-opm's README; no such file was available, and the
three SDK test missions read here are tab-indented, in the [legacy layout](#legacy-layout). One of those two
samples would settle it.

## Conventions

- Member names are PascalCase (`LevelDetails`, `TypeID`, `CenterView`).
- Enumerations are written as names (`"Colony"`, `"Blue"`, `"East"`), not numbers. The SDK's model holds them as
  strings and parses them when the mission starts, each class with its own `GetEnum` helper (`PlayerData`,
  `GameData`, `LevelDetails`, `UnitDataExtensions`, ...), all calling .NET's `Enum.TryParse(string, out T)`. That
  overload is case-sensitive. A name it doesn't know (a misspelling, another case) gives the enum's 0 value
  (`default(T)`); a string of digits gives that number even when the enum doesn't name it, which is how `"90"` in
  [Cargo](#cargo) works.
- `null` appears for unset strings and optional numbers (`"Name":null`, `"BarVariant":null`, `"MaxTubes":null`).
- Whole numbers are written without a fraction, `"Health":1`; a fraction only when there is one, `"Health":0.5`.
- Coordinates (`Position`, `CenterView`, `BaseCenterPt`) are 1-based tile numbers inside the map's playable
  rectangle. The SDK converts them to the game's coordinates as x + xMin − 1 and y + yMin − 1
  (`TethysGame.GetMapCoordinates`), where (xMin, yMin) is the clip rect's top-left corner on a map that doesn't
  wrap and (0, 0) on a wrapping map (`GameMap.Initialize`). With the usual clip rect (x1 = 32, y1 = 0;
  [maps.md](../docs/maps.md#clip-rect)) that is game x = X + 31, y = Y − 1. `GameMap.Initialize` takes (xMin, yMin) from
  the game's clip of (−1, −1) and calls a map wrapping when that x isn't 32. The game's tile x is the file's x + 32
  on a map that doesn't wrap and the file's x on a 512-wide map (measured in op2-rs), so on both kinds of map (X, Y)
  is the `.map` file's cell (X − 1, Y − 1).

## The root object

| Member | Type | Contents |
|---|---|---|
| `SDKVersion` | string | the version tag of the SDK the file was written for; `"0"` in current files |
| `LevelDetails` | object | [mission settings](#leveldetails) |
| `MasterVariant` | [variant](#variants) | what always applies: players, map objects, difficulties, layouts |
| `MissionVariants` | array of [variants](#variants) | alternatives; one is added onto the master variant when the mission starts |
| `Disasters` | array of [disasters](#disaster) | scheduled disasters |
| `Regions` | array of [regions](#region) | named rectangles for triggers |
| `Triggers` | array of [classic triggers](#classic-trigger) | |

OP2MissionEditor has no way to author disasters, regions or triggers and writes them as empty arrays. The older
editor that saved `cmyTut.opm` didn't write `SDKVersion` or `Regions` at all.

### Legacy layout

Older SDK files have no `MasterVariant`: its members `Players`, `TethysGame`, `TethysDifficulties` and
`AutoLayouts` sit in the root. op2-opm moves them into a master variant when the root has no `MasterVariant`, and
keeps any other root member it doesn't know (the SDK test missions have a `Triggers2`). The three SDK test missions
read this way.

## `LevelDetails`

| Member | Type | Contents |
|---|---|---|
| `LevelDescription` | string | the mission's name as shown in the game's lists |
| `MapName` | string | the `.map` file, e.g. `tutorial.map` |
| `TechTreeName` | string | the tech tree sheet, e.g. `MULTITEK.TXT` (see [tech-trees.md](../docs/tech-trees.md)) |
| `MissionType` | string | [`MissionType`](#missiontype), or a campaign mission's number |
| `NumPlayers` | integer | players the mission is for |
| `MaxTechLevel` | integer | the highest tech level available |
| `UnitOnlyMission` | boolean | true for a mission played with units only, no base economy |

The four numbers are those of a mission DLL's `DescBlock` export (mission type, players, maximum tech level, units
only), and the description that of its `LevelDesc`. The game reads only the DLL's exports. The SDK's native plugin
compiles them in (`ExportLevelDetailsFull` in `RequiredExports.h`); the SDK's README says to change them by
editing the built DLL's strings in place. At run time the SDK reads one of these members itself: `MissionType`
decides whether the mission is multiplayer (−4 to −8), which turns off the difficulty merge below
(`MissionLogic`). **Unconfirmed:** that OP2MissionEditor copies these values into the DLL it builds; its source
wasn't available. A DLL it built, next to its `.opm`, would settle it.

## Variants

`MasterVariant` and each element of `MissionVariants`:

| Member | Type | Contents |
|---|---|---|
| `Name` | string or `null` | `null` on the master variant |
| `Players` | array of [players](#player) | one per player slot |
| `TethysGame` | [game data](#tethysgame) | lighting, music, beacons, markers, wreckage |
| `TethysDifficulties` | array of [game data](#tethysgame) | one per difficulty level, added onto `TethysGame` |
| `AutoLayouts` | array of [auto layouts](#autolayout) | bases placed around a centre point |

How variants and difficulties combine, from the SDK's source (`MissionLogic.GetCombinedDifficultyVariant`, the
`Concat` methods of `MissionVariant`, `GameData` and `PlayerData`; op2-opm doesn't merge them):

1. When there are mission variants, one is picked with the game's random number generator and kept in the saved
   game, and added onto the master variant: its `TethysGame` onto the master's, its `TethysDifficulties` and each
   player's `DifficultyResources` onto the master's entry of the same index, its auto layouts appended, and its
   players onto the master's by position. The variant needs at least as many players as the master, and the master
   at least as many difficulty entries as the variant, or the SDK's code fails.
2. In a game that isn't multiplayer, the `TethysDifficulties` entry of the local player's difficulty, if there is
   one, is added onto `TethysGame`.
3. In every game, each player's `DifficultyResources` entry for its difficulty, if there is one, is added onto its
   `Resources`. In a single-player game every player uses the local player's difficulty.

Adding appends the lists (units, walls and tubes, beacons, markers, wreckage, triggers, completed research), adds the
population and stock counts (kids, workers, scientists, ores, food, solar satellites), and replaces the single values
(tech level, morale, free morale, centre view, the lighting switches, the music, and a player's colony, human flag,
AI, `AIImpl`, colour and allies), whatever the added entry holds. One quirk: the list grows by the added `CompletedResearch`'s
length, but only as many ids as the added entry's `TechLevel` are copied into it (`ResourceData.Concat`): a
`TechLevel` above the list's length fails, one below it leaves zeros in place of the last ids.

## `Player`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | player number, 0-based |
| `IsEden` | boolean | Eden (true) or Plymouth (false) |
| `IsHuman` | boolean | human or AI |
| `Color` | string | [`Color`](#color) |
| `BotType` | string | [`BotType`](#bottype): the AI's behaviour |
| `AIImpl` | string | optional, newer SDK files only: which AI implementation runs the slot; empty or `TechCor` for the reference bot, `AIv2` for the other |
| `Allies` | array of integers | the player numbers this player is allied with |
| `Resources` | [resources](#resources) | the starting state |
| `DifficultyResources` | array of [resources](#resources) | one per difficulty level, parallel to `TethysDifficulties` |

## `Resources`

| Member | Type | Contents |
|---|---|---|
| `CenterView` | [point](#point) | the tile the view starts on |
| `CommonOre`, `RareOre` | integers | starting stock |
| `Food` | integer | starting food |
| `Kids`, `Workers`, `Scientists` | integers | starting population |
| `SolarSatellites` | integer | solar power satellites in orbit |
| `TechLevel` | integer | starting tech level |
| `CompletedResearch` | array of integers | tech ids researched at the start |
| `MoraleLevel` | string | [`MoraleLevel`](#moralelevel) |
| `FreeMorale` | boolean | morale doesn't change |
| `Units` | array of [units](#unit) | vehicles and structures |
| `WallTubes` | array of [walls and tubes](#walltube) | |
| `Triggers` | array of [triggers](#trigger) | the player's own triggers |

A new mission's player starts with 10 kids, 14 workers, 8 scientists, 1000 food and `Good` morale (the SDK's and
editor's defaults, as both editor files have them). Those are only defaults for new missions: a reader gives an
absent member its plain default (0, empty), as the SDK's reader does. `cmyTut.opm` has no `Triggers` here.

## `Unit`

A placed vehicle or structure.

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | an id; 0 throughout `cmyTut.opm`, 1 upwards for most units of `opu_12.opm` |
| `TypeID` | string | the unit type, a `map_id` name without `map`: `CargoTruck`, `ConVec`, `Lynx`, `CommandCenter`, `CommonOreMine`, ... |
| `Position` | [point](#point) | its tile |
| `Direction` | string | [`Direction`](#direction) |
| `Health` | number, 0 to 1 | the fraction of hit points left; the SDK sets damage = hit points × (1 − Health) |
| `Lights` | boolean | headlights (vehicles) |
| `CargoType` | string | what it carries; depends on `TypeID` (see [Cargo](#cargo)) |
| `CargoAmount` | integer | how much, or a second id |
| `BarYield`, `BarVariant` | string or `null` | for ore mines: the beacon placed under it ([`BarYield`](#baryield), [`BarVariant`](#barvariant)); `null` for other units |
| `IgnoreLayout` | boolean | for auto layouts |
| `MinDistance`, `SpawnDistance` | integers | for auto layouts |
| `CreateWall` | boolean | for auto layouts |
| `MaxTubes` | integer or `null` | for auto layouts |

An absent `Health` is 1 and an absent `Lights` true: the SDK's reader sets them so before reading
(`OnDeserializing`), and op2-opm does the same.

### Cargo

`CargoType` is written from the cargo's number as the name of an enumeration that depends on the unit:

| Unit | `CargoType` | `CargoAmount` |
|---|---|---|
| `CargoTruck` | a [`TruckCargo`](#truckcargo) name: `Empty`, `Food`, `CommonOre`, ... | the amount |
| `CargoTruck` with a starship module | `Spaceport` | the module's `map_id` number, 88-104 |
| `CargoTruck` with wreckage | `Garbage` | |
| `CargoTruck` with a value that has no `TruckCargo` name | the number as text, e.g. `90` | |
| `ConVec` | the structure kit it carries, a `map_id` name (`CommandCenter`, `Tokamak`, ...) | a second `map_id` (the SDK passes it as the kit's weapon) |
| combat units: `Lynx`, `Panther`, `Tiger`, `Spider`, `Scorpion`, `GuardPost` | the weapon, a `map_id` name (`Laser`, `Microwave`, ...; see [`Weapon`](#weapon)) | |
| anything else | `None` (`map_id` 0) | |

`opu_12.opm` has a cargo truck with `CargoType` `90` and amount 5: a gene bank. The SDK parses `"90"` to the number
90 and passes it to the game's `Unit::SetTruckCargo` (export at 0x476010, body 0x406CD0), which stores the truck's
cargo as one 16-bit word, the amount times 16 plus the cargo's low 4 bits (for wreckage, 9, an amount above 8000
loses 8000 first). 90 is 0x5A, so the truck carries cargo 10, which the game names "Gene Bank" (the cargo name
routine 0x406310 knows 0-10; its case 10 is the string at 0x4E750C). So `"10"` gives the same truck, and any
number whose low 4 bits are 10 does. op2-opm reads `90` and `10` as a gene bank; no file has `10`. The amount keeps
12 bits (0-4095).

Starship module numbers (`map_id`): 88 EDWARD Satellite, 89 Solar Satellite, 90 Ion Drive, 91 Fusion Drive, 92
Command Module, 93 Fueling Systems, 94 Habitat Ring, 95 Sensor Package, 96 Skydock, 97 Stasis Systems, 98 Orbital
Package, 99 Phoenix Module, 100 Rare Metals Cargo, 101 Common Metals Cargo, 102 Food Cargo, 103 Evacuation Module,
104 Children Module.

## `WallTube`

| Member | Type | Contents |
|---|---|---|
| `TypeID` | string | `Tube`, `Wall`, `LavaWall` or `MicrobeWall` |
| `Position` | [point](#point) | its tile |

The three SDK test missions write the tile as `Location` instead (2 to 86 times each). op2-opm reads either and
writes `Position`.

## `TethysGame`

The game data of a variant, also each element of `TethysDifficulties`.

| Member | Type | Contents |
|---|---|---|
| `DaylightEverywhere` | boolean | no night |
| `DaylightMoves` | boolean | the day and night band moves; true in a new mission |
| `InitialLightLevel` | integer | where the band starts |
| `MusicPlayList` | object | `SongIDs` (array of integers, the songs in order) and `RepeatStartIndex` (where in the list to start again after the last song); a new mission has `SongIDs` `[0]` |
| `Beacons` | array of [beacons](#beacon) | |
| `Markers` | array of [markers](#marker) | |
| `Wreckage` | array of [wreckage](#wreckage) | |
| `Triggers` | array of [triggers](#trigger) | |
| `WallTubes` | array of [walls and tubes](#walltube) | belonging to nobody; written by some SDK 3.0 builds, not part of the SDK's current model |

`cmyTut.opm` has no `Triggers` here.

### `Beacon`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | |
| `MapID` | string | `MiningBeacon`, `Fumarole` or `MagmaVent` |
| `OreType` | string | [`OreType`](#oretype); for mining beacons |
| `BarYield` | string | [`BarYield`](#baryield) |
| `BarVariant` | string | [`BarVariant`](#barvariant) |
| `Position` | [point](#point) | |

### `Marker`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | |
| `MarkerType` | string | [`MarkerType`](#markertype) |
| `Position` | [point](#point) | |

### `Wreckage`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | |
| `TechID` | integer | the tech the wreckage gives, as a number (e.g. 8000), unlike the unit names elsewhere |
| `IsVisible` | boolean | shown before it is found |
| `Position` | [point](#point) | |

The SDK's model declares `TechID` a string, but every file has a number there; op2-opm reads only a number.

## Small types

### `Point`

`{ "X": integer, "Y": integer }`: a tile (see [Conventions](#conventions)).

### `DataRect`

`MinX`, `MinY`, `MaxX`, `MaxY`: integers, a tile rectangle, corners included.

### `AutoLayout`

| Member | Type | Contents |
|---|---|---|
| `PlayerID` | integer | whose base |
| `BaseCenterPt` | [point](#point) | the centre the units are placed around |
| `Units` | array of [units](#unit) | placed using their `IgnoreLayout`, `MinDistance`, `SpawnDistance`, `CreateWall` and `MaxTubes` |

### `Disaster`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | |
| `Type` | string | `Meteor`, `Earthquake`, `Lightning`, `Tornado`, `Eruption` or `Blight` |
| `SrcRect`, `DestRect` | [rects](#datarect) | where it starts, and where a moving one goes |
| `Size` | integer | its strength |
| `Duration` | integer | |
| `StartTime`, `EndTime` | integers | the time window |
| `MinDelay`, `MaxDelay` | integers | the range of the delay between repeats |

### `Region`

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | in the SDK's model; op2-opm keeps it as an unknown member |
| `Name` | string | the name triggers use |
| `Rect` | [rect](#datarect) | |

## Triggers

Two kinds, not interchangeable.

### Classic trigger

The root's `Triggers`: flat records, most members meaningful only for some `Type`s.

| Member | Type | Contents |
|---|---|---|
| `ID`, `TriggerID` | integers | its id, and the id of the game trigger it makes or links to |
| `Type` | string | `Victory`, `Failure`, `Research`, `Time`, `Set`, ... |
| `Enabled`, `OneShot` | booleans | starts enabled (true when absent); fires once |
| `PlayerID` | integer | the player it watches |
| `Message` | string | |
| `Count`, `CompareType` | integer, string | a count and how to compare it |
| `UnitType` | string | a `map_id` name |
| `Time`, `MinTime`, `MaxTime` | integers | times, in ticks |
| `TechID` | integer | for research triggers |
| `X`, `Y`, `Width`, `Height` | integers | a point or rectangle |
| `CargoOrWeaponType`, `ResourceType`, `CargoType` | strings | selectors |
| `CargoAmount` | integer | |
| `ChildTriggerIDs` | array of integers | the triggers a `Set` trigger combines |
| `NeededTriggers` | integer | how many of them must fire (0: all) |
| `Actions` | array | each `Type`, `Message`, `PlayerID`, `SoundID` |

### `Trigger`

The `Triggers` of `TethysGame` and of `Resources`: an event, conditions and actions.

| Member | Type | Contents |
|---|---|---|
| `ID` | integer | |
| `Enabled` | boolean | true when absent |
| `EventType` | string | when it is checked: `OnGameTick`, `OnVehicleEnteredRegion`, ... |
| `Condition` | array | the conditions (the name is singular though it holds a list): `Type`, `Subject`, `Comparison`, `Value`, `Value2`, `Value3`, all strings |
| `Actions` | array | `Type`, `Modifier` (`Set`, `Add`, `Subtract`), `Subject`, `Subject2`, `SubjectPlayer`, `SubjectRegion` (strings), `SubjectQuantity` (integer), `Value` to `Value5` (strings) |

What the `Value`s mean depends on the condition's or action's `Type`. The SDK defines the lists of event,
condition and action types (`MissionReader/Json/Triggers`); op2-opm keeps them as strings.

## Enumerations

The numbers are the game's, where the value is passed to it.

### `MissionType`

| Name | Value | File name prefix |
|---|---|---|
| `Colony` | −1 | `c` |
| `AutoDemo` | −2 | `a` |
| `Tutorial` | −3 | `t` |
| `MultiLandRush` | −4 | `mu`*N* |
| `MultiSpaceRace` | −5 | `mf`*N* |
| `MultiResourceRace` | −6 | `mr`*N* |
| `MultiMidas` | −7 | `mm`*N* |
| `MultiLastOneStanding` | −8 | `ml`*N* |

*N* is `NumPlayers`. Measured: the first word of the `DescBlock` export of the 106 mission DLLs in the game folder
is −1 in the six colony games (`CEP1`, `CES1`, `CPP1`, `CPS1`, `CPS2`, `us_them`), −2 in the three demos, −3 in the 14 tutorials, −4 to −8 in the `mu`, `mf`, `mr`,
`mm` and `ml` games, and the mission number (1 to 12) in the Eden and Plymouth campaign DLLs. A campaign mission
writes its number in `MissionType`, e.g. `"10"`. The prefixes are the editor's (`file_prefix` in op2-opm).

### `Color`

`Blue` 0, `Red` 1, `Green` 2, `Yellow` 3, `Cyan` 4, `Magenta` 5, `Black` 6 (the SDK's `PlayerColor`; a new
mission gives player *n* the *n*-th, `PlayerData.GetPlayerColor`). OP2OpmTools' colour table adds `White` 7, but
the SDK names no colour 7: it reads `White`, like any name it doesn't know, as `Blue` (0) before passing the
colour to `Player::SetColorNumber` (`MissionLogic`). The SDK's default for a player beyond the seventh, `Clear`,
reads as `Blue` the same way. Don't write `White`.

### `BotType`

`None` (no AI; humans), `PopulationGrowth`, `LaunchStarship`, `EconomicGrowth`, `Passive`, `Defender`, `Balanced`,
`Aggressive`, `Harassment`, `Wreckless` (the SDK's `BotType`, in its order). A new mission gives player 0 `None` and
the others `Balanced`, as in `cmyTut.opm`.

### `MoraleLevel`

`Excellent` 0, `Good` 1, `Fair` 2, `Poor` 3, `Terrible` 4.

### `Direction`

`East` 0, `SouthEast` 1, `South` 2, `SouthWest` 3, `West` 4, `NorthWest` 5, `North` 6, `NorthEast` 7: the facing,
clockwise from east, in the SDK's spelling. `opu_12.opm` spells three of them `Southeast`, `Northwest`,
`Northeast`; the SDK's case-sensitive parsing reads them as 0, so those units start facing east.

### `OreType`, `BarYield`, `BarVariant`

| Enum | Values |
|---|---|
| `OreType` | `Random` −1, `Common` 0, `Rare` 1 |
| `BarYield` | `Random` −1, `Bar3` 0, `Bar2` 1, `Bar1` 2 |
| `BarVariant` | `Random` −1, `Variant1` 0, `Variant2` 1, `Variant3` 2 |

### `MarkerType`

`Circle` 0, `DNA` 1, `Beaker` 2 (the SDK's). They are the only types the game knows: `TethysGame::PlaceMarker`
(0x478BB0) turns 0, 1 and 2 into the animations with game ids 1865, 2169 and 2170 (`op2_art.prt` records 1764,
2068 and 2069; see [sprites.md](../docs/sprites.md#game-ids)) and has no value for any other type (it then uses
whatever an unset local variable holds). op2-opm also knows `Common` and `Rare`, from the editor's marker art; no
file has them, the SDK reads them as `Circle`, and they shouldn't be written.

### `TruckCargo`

`Empty` 0, `Food` 1, `CommonOre` 2, `RareOre` 3, `CommonMetal` 4, `RareMetal` 5, `CommonRubble` 6, `RareRubble` 7,
`Spaceport` 8 (a starship module), `Garbage` 9 (wreckage). `opu_12.opm` has an empty truck written `Empty`.

### `Weapon`

`map_id` names: `AcidCloud`, `EMP`, `Laser`, `Microwave`, `RailGun`, `RPG`, `Starflare`, `Supernova`, `ESG`,
`Stickyfoam`, `ThorsHammer`.

## The editor's text layout

OP2MissionEditor's files, as `opu_12.opm` and `cmyTut.opm` show them and op2-opm writes them:

- The file starts with a CRLF, then the root `{`. It ends with the root's `}` and no line break.
- Lines end in CRLF and are indented with tabs, one per level.
- Members are sorted by name in ordinal (byte) order, so `ID` comes before `IgnoreLayout` and `IsEden`.
- A member is `"Name":value` with no space. Members are separated by a comma at the end of the line.
- An object or array value starts on the line after its member name, at the member's indent: `"Position":`, then
  `{` on the next line, its members one level deeper, `}` back at the member's indent.
- Inside an array, each object element is preceded by a blank line (indented one level deeper than the `[`), and
  number elements are one per line. An empty array is `[`, a blank indented line, `]`.
- Every member of the model is written, `null`s included. op2-opm also writes members the file lacked (an older
  editor's file gains `SDKVersion`, `Regions` and the `Triggers` lists); `AIImpl` and `TethysGame.WallTubes` only
  when they have a value.

So an empty mission's root starts:

```
{
	"Disasters":
	[
		
	],
	"LevelDetails":
	{
		"LevelDescription":"My mission",
		...
```

op2-opm writes the editor's files back as the same JSON. Byte for byte they differ in two ways: `opu_12.opm`'s
direction names are written `SouthEast`, `NorthWest`, `NorthEast`, and some of its blank lines have no tabs (those
from line 942 on, all but one); the rule the editor's formatter follows there isn't known. A JSON reader ignores
both. Unknown members are written back among the others in name order.

## What op2-opm accepts

- UTF-8, with or without a byte-order mark.
- Either layout, members in any order; `//` and `/* */` comments and trailing commas, as Newtonsoft.Json accepts.
- An absent member, or `null`, as its default: 0, false, empty, `None` for `CargoType`, health 1 and lights on for
  units, enabled for triggers, and the first value listed in op2-opm for each enumeration.
- Enumeration names in any case (`Southeast` reads as `SouthEast`); an unknown name, or a number where a name
  belongs, is kept as written and written back.
- Members it doesn't know, kept in their object and written back.
- The legacy root layout, and `Location` for a wall or tube's tile.

## Making one

Write the editor's layout (or any JSON; the SDK reads both), with at least `LevelDetails`, `MasterVariant` with a
`Players` entry and a `TethysGame`, and empty arrays for the rest. Use 1-based coordinates inside the map's playable
rectangle, the SDK's spellings for every enumeration, and `CargoType` names that fit the unit (`Empty` for an empty
truck, a structure for a ConVec, a weapon for a combat unit, `None` otherwise). The `.map` named in `MapName` and a
mission DLL built with the SDK go beside the file.

## Sources

- op2-opm `model.rs` (members and defaults), `enums.rs`, `lib.rs` (reading rules, cargo), `write.rs` (the editor's
  layout), its README and `tests/opm.rs`.
- OP2DotNetMissionSDK source (OutpostUniverse; Leviathan's copy): `MissionReader/MissionReader.cs` (the
  serializer), `MissionReader/Json` (member names, defaults, `Concat`, the `GetEnum` helpers), `MissionSDK/MissionLogic`
  (how variants and difficulties combine), `Outpost2DLL` enums (`MissionType`, `PlayerColor`, `TruckCargo`,
  `MoraleLevel`, `UnitDirection`, `MarkerType`, `map_id`), `UnitDataExtensions` (cargo by unit type, health),
  `TethysGame.GetMapCoordinates` and `GameMap.Initialize` (coordinates), the native plugin's `RequiredExports.h` and
  the SDK's README (the DLL's level details).
- Disassembly of GOG's `Outpost2.exe` with an iced-x86 probe: `Unit::SetTruckCargo` (0x476010, 0x406CD0), the cargo
  names (0x406310), `TethysGame::PlaceMarker` (0x478BB0).
- op2-rs (bufke's reimplementation): the measured tile offset between the game's coordinates and the `.map`'s.
- OP2OpmTools' `OPM_FORMAT.md` (Leviathan): the two writer styles, the bar yield and variant values, the colour
  table.
- Measured: the `DescBlock` of the 106 mission DLLs of GOG Outpost 2; the two editor missions read and rewritten
  with op2-opm and compared; the three SDK test missions read.
