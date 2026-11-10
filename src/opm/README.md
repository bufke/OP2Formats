# op2-formats: `opm`

Reads and writes `.opm` files, the JSON missions [OP2MissionEditor](https://github.com/leviathan400/OP2MissionEditor)
saves next to each mission's `.map` and plugin DLL. The format is described in OP2OpmTools' `OPM_FORMAT.md`; the data
model follows the OP2DotNetMissionSDK's `MissionReader/Json` classes.

Behind the `opm` feature: `op2-formats = { version = "0.2", features = ["opm"] }`.

```rust
let mut mission = op2_formats::opm::Mission::open("cmyTut.opm")?;
for player in &mission.master_variant.players {
    for unit in &player.resources.units {
        println!("{} at {:?}, {:?}", unit.type_id, unit.position, unit.cargo());
    }
}
mission.level_details.description = "Edited".into();
mission.save("cmyTut.opm")?;
```

## Reading

Any known writer's file reads:

- OP2MissionEditor's (tabs, members in alphabetical order, every member written) and the SDK 3.0 tools' (two-space
  indents, declaration order, defaults left out). Members are read by name, in any order.
- An absent member, or `null`, takes its default; for units that means full health and lights on, as the SDK reads
  them.
- Members the model doesn't know (`Comment`, `SpawnRect`, `Triggers2`, ...) are kept in each struct's `extra` and
  written back.
- Enum names match ignoring case (`Southeast` reads as `SouthEast`); an unknown name is kept as `Other`.
- A wall or tube's tile is read from `Position` or `Location`.
- The older SDK layout, with `Players` and `TethysGame` at the top level, is moved into the master variant.
- Comments and trailing commas, which Newtonsoft.Json (the SDK tools' reader) accepts.

`Unit::cargo` decodes `CargoType` with `CargoAmount`: truck cargo, a starship module (`Spaceport`, the module id in
`CargoAmount`), wreckage (`Garbage`), a gene bank (written as the number `90`; a number is read by its low 4 bits,
as the game stores truck cargo), a ConVec's structure kit (with `CargoAmount` as its weapon), or a weapon.

Enum values follow the SDK and the game: `MissionType` is the `DescBlock` value (`Colony` −1 to
`MultiLastOneStanding` −8; `number()` also reads a campaign mission's `"10"`), `MoraleLevel` counts from `Excellent`
0, and `BotType` has the SDK's ten presets. Names the SDK doesn't define (`White`, the `Common` and `Rare` markers) read
as `Other` and are written back as they were.

## Writing

`Mission::to_opm` and `Mission::save` write OP2MissionEditor's layout: a line break first, tab indents, CRLF line
ends, members in alphabetical order, every member written, names in the SDK's spelling. Files the editor saved come
back as the same JSON. They also come back byte for byte, apart from two things: the tabs on some blank lines, which
follow a formatter quirk not worked out, and names in other spellings, which are written the SDK's way (`SouthEast`
for `Southeast`).

## Tests

`tests/data/opm` holds two missions saved with OP2MissionEditor (`opu_12.opm`, `cmyTut.opm`). `tests/opm.rs` checks
that they read and write back the same JSON. It also covers every reading rule above on hand-written text. With
`OP2_OPM_DIR` set to a folder of `.opm` files, it reads and writes back every one. That passed with 11 files in four
layouts: OP2MissionEditor's, OP2DotNetMissionSDK's test missions (including the older layout and a trailing comma),
OP2 Remake's missions, and two files with each member on one line.

`cargo run --features opm --example opm_rewrite -- in.opm out.opm` reads a file and writes it in the editor's layout.

## Not yet

Merging a variant and a difficulty onto the master variant, converting `.opm` coordinates to map and in-game tiles,
mapping unit names to op2-formats' unit art, and checking a mission against its map and tech tree. See
[NOTES.md](../../NOTES.md).
