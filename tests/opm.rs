//! Reading and writing `.opm` files. `tests/data/opm` holds two missions saved by OP2MissionEditor; `$OP2_OPM_DIR`, when
//! set, names a folder whose `.opm` files (any writer's) are all read and written back too.

use op2_formats::opm::*;
use serde_json::Value;

fn data(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/opm/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Where two JSON values differ, as paths (empty when they're equal).
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => differences(p, q, &format!("{path}.{k}"), out),
                    // Members older editors didn't write yet come back with their defaults.
                    (Some(p), None) if is_default(p) => {}
                    (p, q) => out.push(format!("{path}.{k}: {p:?} vs {q:?}")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                differences(p, q, &format!("{path}[{i}]"), out);
            }
        }
        // Names are read ignoring case and written in the SDK's spelling (opu_12's `Southeast` comes back
        // `SouthEast`, the SDK's `UnitDirection` name).
        (Value::String(p), Value::String(q)) if p.eq_ignore_ascii_case(q) => {}
        (p, q) if p != q => out.push(format!("{path}: {p} vs {q}")),
        _ => {}
    }
}

fn is_default(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.values().all(is_default),
    }
}

/// Writing a mission and reading it back gives the same mission.
fn round_trips(m: &Mission) {
    let again = Mission::parse(&m.to_opm()).unwrap();
    assert_eq!(&again, m);
}

#[test]
fn editor_files_read_and_write_back_the_same_json() {
    for name in ["opu_12.opm", "cmyTut.opm"] {
        let text = data(name);
        let m = Mission::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        round_trips(&m);
        // Every member the editor wrote comes back with the same value.
        let original: Value = serde_json::from_str(&text).unwrap();
        let written: Value = serde_json::from_str(&m.to_opm()).unwrap();
        let mut diff = Vec::new();
        differences(&written, &original, "", &mut diff);
        assert!(diff.is_empty(), "{name}, written vs original:\n{}", diff[..diff.len().min(20)].join("\n"));
        let same_bytes = m.to_opm() == text;
        eprintln!("{name}: written {} byte for byte", if same_bytes { "the same" } else { "differently" });
    }
}

#[test]
fn opu_12_contents() {
    let m = Mission::parse(&data("opu_12.opm")).unwrap();
    let d = &m.level_details;
    assert_eq!((d.map_name.as_str(), d.mission_type.clone(), d.num_players), ("opu_12.map", MissionType::Colony, 2));
    let p0 = &m.master_variant.players[0];
    assert!(p0.is_eden && p0.is_human && p0.color == Color::Blue);
    assert_eq!(p0.resources.center_view, Point { x: 17, y: 17 });
    assert_eq!(p0.resources.common_ore, 2000);
    let cc = &p0.resources.units[0];
    assert_eq!((cc.type_id.as_str(), cc.position, cc.health), ("CommandCenter", Point { x: 15, y: 15 }, 1.0));
    assert_eq!((cc.bar_variant.clone(), cc.cargo()), (None, Cargo::None));
    let game = &m.master_variant.tethys_game;
    assert_eq!(game.wreckage[0].tech_id, 8000);
    assert_eq!(game.markers.iter().map(|k| k.marker_type.clone()).collect::<Vec<_>>(), [MarkerType::Circle, MarkerType::DNA, MarkerType::Beaker]);
    assert_eq!(m.sdk_version, "0");
}

/// An SDK 3.0-style file: two-space indents, declaration order, defaults left out, `null`s, unknown members, a wall
/// tube at `Location`, a gene bank written as 90 and a facing in the wrong case.
const SDK_STYLE: &str = r#"{
  "SDKVersion": "3",
  "LevelDetails": { "MapName": "on2_01.map", "MissionType": "MultiLastOneStanding", "NumPlayers": 2, "Future": 1 },
  "MasterVariant": {
    "Name": null,
    "Players": [
      { "ID": 1, "IsEden": false, "Color": "Red", "BotType": "Balanced", "AIImpl": "AIv2",
        "Resources": {
          "Units": [
            { "TypeID": "CargoTruck", "Position": { "X": 3, "Y": 4 }, "Direction": "Southeast", "CargoType": "90" },
            { "TypeID": "Lynx", "CargoType": "Laser", "Health": 0.5, "Lights": false },
            { "TypeID": "CargoTruck", "CargoType": "Spaceport", "CargoAmount": 92 },
            { "TypeID": "CargoTruck", "CargoType": "RareOre", "CargoAmount": 1000 },
            { "TypeID": "CargoTruck", "CargoType": "Garbage", "BarYield": null }
          ],
          "WallTubes": [ { "TypeID": "Tube", "Location": { "X": 19, "Y": 14 } } ],
          "MoraleLevel": "Excellent"
        } }
    ],
    "TethysGame": { "Beacons": [ { "MapID": "Fumarole", "Position": { "X": 1, "Y": 2 } } ], "MusicPlayList": null }
  },
  "Triggers": [ { "Type": "Victory", "Actions": [ { "Type": "Message", "Message": "Won" } ] } ]
}"#;

#[test]
fn sdk_style_files_read_with_defaults_and_unknowns() {
    let m = Mission::parse(SDK_STYLE).unwrap();
    assert_eq!(m.level_details.extra["Future"], 1, "unknown members are kept");
    let p = &m.master_variant.players[0];
    assert_eq!((p.id, p.color.clone(), p.ai_impl.as_deref()), (1, Color::Red, Some("AIv2")));
    let units = &p.resources.units;
    assert_eq!((units[0].direction.clone(), units[0].health, units[0].lights), (Direction::SouthEast, 1.0, true));
    assert_eq!(units[0].cargo(), Cargo::GeneBank);
    assert_eq!((units[1].cargo(), units[1].health, units[1].lights), (Cargo::Weapon(Weapon::Laser), 0.5, false));
    assert_eq!(units[2].cargo(), Cargo::StarshipModule(92));
    assert_eq!(units[3].cargo(), Cargo::Truck { cargo: TruckCargo::RareOre, amount: 1000 });
    assert_eq!(units[4].cargo(), Cargo::Wreckage);
    assert_eq!(p.resources.wall_tubes[0].position, Point { x: 19, y: 14 });
    assert_eq!(p.resources.morale_level, MoraleLevel::Excellent);
    assert_eq!(m.master_variant.tethys_game.beacons[0].kind, BeaconKind::Fumarole);
    assert!(m.triggers[0].enabled, "an absent Enabled is the SDK's true");
    assert_eq!(m.triggers[0].actions[0].message, "Won");
    round_trips(&m);

    // Written in the editor's layout: `Position` for the wall tube, the canonical facing, every member present.
    let text = m.to_opm();
    assert!(text.starts_with("\r\n{\r\n\t\"Disasters\":\r\n\t[\r\n\t\t\r\n\t],"), "{}", &text[..60]);
    assert!(text.contains("\"Position\":") && !text.contains("\"Location\""));
    assert!(text.contains("\"Direction\":\"SouthEast\"") && text.contains("\"AIImpl\":\"AIv2\""));
    assert!(text.contains("\"Health\":0.5") && text.contains("\"Health\":1,"));
}

#[test]
fn old_sdk_layout_moves_into_the_master_variant() {
    let m = Mission::parse(r#"{ "Players": [ { "ID": 0 } ], "TethysGame": { "DaylightMoves": true }, "Triggers2": [ 1 ] }"#).unwrap();
    assert_eq!(m.master_variant.players.len(), 1);
    assert!(m.master_variant.tethys_game.daylight_moves);
    assert_eq!(m.extra["Triggers2"], serde_json::json!([1]), "other old members stay");
}

#[test]
fn unknown_enum_names_are_kept() {
    let m = Mission::parse(r#"{ "LevelDetails": { "MissionType": "SomethingNew" }, "MasterVariant": { "Players": [ { "Color": 3 } ] } }"#).unwrap();
    assert_eq!(m.level_details.mission_type, MissionType::Other("SomethingNew".into()));
    assert_eq!(m.master_variant.players[0].color, Color::Other("3".into()));
    assert!(m.to_opm().contains("\"MissionType\":\"SomethingNew\""));
    assert_eq!(MissionType::MultiSpaceRace.file_prefix(4), "mf4");
    assert_eq!(Direction::NorthEast.value(), Some(7));
}

#[test]
fn comments_and_trailing_commas_read_as_newtonsoft_does() {
    let text = "{ // a comment\n \"LevelDetails\": { \"MapName\": \"http://x/a.map\", /* block\n comment */ \"NumPlayers\": 2, },\n \"Regions\": [ { \"Name\": \"a,]\" }, ], }";
    let m = Mission::parse(text).unwrap();
    assert_eq!((m.level_details.map_name.as_str(), m.level_details.num_players), ("http://x/a.map", 2));
    assert_eq!(m.regions[0].name, "a,]");
}

#[test]
fn errors_say_where() {
    let err = Mission::parse("{ \"LevelDetails\": ").unwrap_err().to_string();
    assert!(err.contains("line"), "{err}");
    let err = Mission::parse(r#"{ "LevelDetails": { "NumPlayers": "two" } }"#).unwrap_err().to_string();
    assert!(err.contains("invalid type"), "{err}");
}

#[test]
fn every_opm_in_op2_opm_dir() {
    let Some(dir) = std::env::var_os("OP2_OPM_DIR") else {
        eprintln!("skipping: set OP2_OPM_DIR to a folder of .opm files");
        return;
    };
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("opm")) {
            continue;
        }
        let m = Mission::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        round_trips(&m);
        seen += 1;
    }
    assert!(seen > 0, "no .opm files in {}", dir.to_string_lossy());
    eprintln!("{seen} files read and written back");
}
