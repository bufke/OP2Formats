//! Mission DLLs against a real install (`$OP2_DIR`, GOG Outpost 2 with OPU 1.4.1): the counts and values
//! `docs/mission-dlls.md` gives for the 159 missions.

use std::collections::BTreeMap;

use op2_formats::mission::{self, MissionDll, MissionType};
use op2_formats::testdata;

fn file_name(path: &std::path::Path) -> String {
    path.file_stem().unwrap().to_string_lossy().to_ascii_lowercase()
}

#[test]
fn every_mission_in_the_install() {
    let Some(files) = testdata::game_files() else { return };
    let missions = mission::missions(&files);
    assert_eq!(missions.len(), 159, "106 Sierra missions and 53 under OPU\\maps");

    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    for (_, m) in &missions {
        let key = match m.mission_type {
            MissionType::Campaign(_) => "campaign".to_string(),
            other => format!("{other:?}"),
        };
        *by_type.entry(key).or_default() += 1;
    }
    let expected = [
        ("campaign", 24),
        ("Colony", 24),
        ("AutoDemo", 3),
        ("Tutorial", 14),
        ("LandRush", 16),
        ("SpaceRace", 17),
        ("ResourceRace", 8),
        ("Midas", 11),
        ("LastOneStanding", 42),
    ];
    assert_eq!(by_type, expected.iter().map(|&(k, v)| (k.to_string(), v)).collect(), "missions by type");

    for (path, m) in &missions {
        let name = file_name(path);
        assert!(m.has_export("InitProc") && m.has_export("AIProc"), "{name}: InitProc and AIProc");
        assert!(m.warnings.is_empty(), "{name}: {:?}", m.warnings);
        assert!(!m.level_desc.is_empty() && !m.map_name.is_empty() && !m.techtree_name.is_empty(), "{name}");
        assert!(!m.export("DescBlock").unwrap().is_code, "{name}: DescBlock is data");
        assert!(m.export("InitProc").unwrap().is_code, "{name}: InitProc is code");
        if let MissionType::Campaign(n) = m.mission_type {
            assert_eq!(name[1..].parse::<i32>().ok(), Some(n as i32), "{name}: campaign number");
        }
    }

    let unit_only: Vec<String> = missions.iter().filter(|(_, m)| m.unit_only).map(|(p, _)| file_name(p)).collect();
    assert_eq!(
        unit_only,
        ["cresesc", "e04", "e06", "e09", "e11", "ml6_sr", "mm4_x02", "mm4_x05", "mu2resc", "p04", "p06", "p09", "p10", "t02"],
        "unit-only missions"
    );

    let with_ex = missions.iter().filter(|(_, m)| m.extra_ai_players.is_some()).count();
    assert_eq!(with_ex, 30, "DescBlockEx in 2 Sierra DLLs and 28 others");

    let mut trees: BTreeMap<String, usize> = BTreeMap::new();
    for (_, m) in &missions {
        let tree = m.techtree_name.to_ascii_uppercase();
        let key = if ["MULTITEK.TXT", "PLY_TEK.TXT", "EDENTEK.TXT", "TUTORTEK.TXT"].contains(&tree.as_str()) { tree } else { "other".into() };
        *trees.entry(key).or_default() += 1;
    }
    let expected = [("EDENTEK.TXT", 13), ("MULTITEK.TXT", 90), ("PLY_TEK.TXT", 16), ("TUTORTEK.TXT", 12), ("other", 28)];
    assert_eq!(trees, expected.iter().map(|&(k, v)| (k.to_string(), v)).collect(), "tech trees");
}

#[test]
fn particular_missions() {
    let Some(files) = testdata::game_files() else { return };
    let open = |name: &str| {
        let path = files.loose_path(name).unwrap_or_else(|| panic!("{name} not in the install"));
        MissionDll::open(path).unwrap()
    };

    let m = open("us_them.dll");
    assert_eq!((m.mission_type, m.players), (MissionType::Colony, 4));
    assert_eq!(m.level_desc, "This tests building all the buildings and units.");
    assert!(!m.has_export("StatusProc"), "the one Sierra DLL without StatusProc");

    let m = open("e01.dll");
    assert_eq!((m.mission_type, m.max_tech_level, m.level_desc.as_str()), (MissionType::Campaign(1), 1, "Eden Mission #1"));
    assert!(m.has_export("GetSaveRegions"));

    assert_eq!(open("ml2_21.dll").extra_ai_players, Some(1));

    // The name says 4 players, DescBlock 5; the lobby checks them separately.
    let m = open("mr4Yuko.dll");
    assert_eq!(mission::multiplayer_name("mr4Yuko.dll"), Some((MissionType::ResourceRace, 4)));
    assert_eq!((m.mission_type, m.players), (MissionType::ResourceRace, 5));
}

#[test]
fn sierra_multiplayer_names_match_their_type() {
    let Some(files) = testdata::game_files() else { return };
    let mut checked = 0;
    for (path, m) in mission::missions(&files) {
        // The Sierra copies: in the game folder, or OPU's copies under base\maps.
        let sierra = path.parent() == Some(files.root()) || path.components().any(|c| c.as_os_str().eq_ignore_ascii_case("base"));
        if !sierra || !m.mission_type.is_multiplayer() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let (kind, players) = mission::multiplayer_name(&name).unwrap_or_else(|| panic!("{name}: not an m-name"));
        assert_eq!((kind, players as i32), (m.mission_type, m.players), "{name}");
        checked += 1;
    }
    assert_eq!(checked, 59, "13 + 8 + 7 + 6 + 25 Sierra multiplayer missions");
}
