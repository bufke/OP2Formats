//! JSON maps against real files: `$OP2_MAPJSON` names a JSON map written by OP2MapJsonTools (PerRowPadded), or a
//! folder of them, which must read and write back byte for byte; `$OP2_DIR` an Outpost 2 install, whose every map
//! converts to JSON and back. Both skip without their variable (`OP2_REQUIRE_DATA=1` makes the game's a failure).

use op2_formats::{GameFiles, Map};
use op2_formats::mapjson::*;

fn game() -> Option<GameFiles> {
    let Some(dir) = std::env::var_os("OP2_DIR").filter(|d| !d.is_empty()) else {
        assert!(std::env::var_os("OP2_REQUIRE_DATA").is_none(), "OP2_REQUIRE_DATA is set but OP2_DIR isn't");
        eprintln!("skipping: set OP2_DIR to an Outpost 2 install");
        return None;
    };
    Some(GameFiles::open(dir).unwrap())
}

/// Panics showing where two texts first differ.
fn same_text(ours: &str, theirs: &str, what: &str) {
    if ours != theirs {
        let at = ours.bytes().zip(theirs.bytes()).position(|(a, b)| a != b).unwrap_or(ours.len().min(theirs.len()));
        panic!(
            "{what}: differs at byte {at} (line {}): ours {:?}, the file's {:?}",
            theirs[..at].lines().count(),
            &ours[at.saturating_sub(40)..(at + 40).min(ours.len())],
            &theirs[at.saturating_sub(40)..(at + 40).min(theirs.len())]
        );
    }
}

/// Each OP2MapJsonTools file writes back byte for byte and makes a map with the same tileset. When `$OP2_DIR` is
/// set too and the file's `map` is a game map with the same tiles, our own conversion of that map, with the file's
/// header texts, is the file byte for byte (checked with eden03.json), and the file converts back to its bytes.
#[test]
fn op2mapjsontools_files_write_back_byte_for_byte() {
    let Some(path) = std::env::var_os("OP2_MAPJSON").map(std::path::PathBuf::from) else {
        eprintln!("skipping: set OP2_MAPJSON to a JSON map written by OP2MapJsonTools, or a folder of them");
        return;
    };
    let paths: Vec<_> = if path.is_dir() {
        let mut p: Vec<_> = std::fs::read_dir(&path).unwrap().flatten().map(|e| e.path()).collect();
        p.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")));
        p
    } else {
        vec![path]
    };
    let files = std::env::var_os("OP2_DIR").map(|d| GameFiles::open(d).unwrap());
    for path in paths {
        let what = path.display().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let json = MapJson::parse(&text).unwrap_or_else(|e| panic!("{what}: {e}"));
        same_text(&json.to_json(), &text, &what);
        let map = json.to_map(None).unwrap();
        assert!(MapJson::from_map(&map, &json.header.map).tileset == json.tileset, "{what}");

        let Some(original) = files.as_ref().and_then(|f| f.read(&json.header.map).ok()).map(|(d, _)| d) else { continue };
        let game_map = Map::parse(&original).unwrap();
        let mut ours = MapJson::from_map(&game_map, &json.header.map);
        if ours.tiles != json.tiles {
            continue; // another map of the same name
        }
        ours.header.name.clone_from(&json.header.name);
        ours.header.author.clone_from(&json.header.author);
        ours.header.notes.clone_from(&json.header.notes);
        same_text(&ours.to_json(), &text, &format!("{what}, against our conversion of {}", json.header.map));
        assert!(json.to_map(Some(&game_map)).unwrap().write() == original, "{what}: back to {}'s bytes", json.header.map);
        eprintln!("{what}: the same as our conversion of {}", json.header.map);
    }
}

/// Every map to JSON text, back from the text, and to a map again with the original as the base: the same bytes.
/// Without a base, the same map but for what the JSON doesn't carry.
#[test]
fn every_game_map_round_trips() {
    let Some(files) = game() else { return };
    let mut count = 0;
    for name in files.map_names() {
        let (original, _) = files.read(&name).unwrap();
        let map = Map::parse(&original).unwrap();
        let json = MapJson::from_map(&map, &name);
        let text = json.to_json();
        let back = MapJson::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(back == json, "{name}: the JSON reads back the same");
        assert!(back.to_json() == text, "{name}: and writes the same text");
        assert!(back.to_map(Some(&map)).unwrap().write() == original, "{name}: back to the same .map bytes");

        let bare = back.to_map(None).unwrap();
        assert_eq!((bare.width, bare.height, bare.clip_rect), (map.width, map.height, map.clip_rect), "{name}");
        assert!(bare.mappings.len() == map.mappings.len() && bare.terrain_types == map.terrain_types, "{name}");
        assert_eq!(bare.tilesets.len(), DEFAULT_TILESET_SLOTS.max(map.tilesets.len()), "{name}");
        assert!(bare.tile_groups.is_empty());
        for y in 0..map.height {
            for x in 0..map.width {
                assert_eq!(bare.tile_bits(x, y), map.tile_bits(x, y) & 0xFFFF, "{name} ({x}, {y})");
            }
        }
        count += 1;
    }
    assert!(count > 80, "{count} maps");
    eprintln!("{count} maps round-trip");
}

#[test]
fn clip_rect_is_written_only_when_not_the_default() {
    assert_eq!(default_clip_rect(128, 64), ClipRect { x1: 32, y1: 0, x2: 159, y2: 62 });
    assert_eq!(default_clip_rect(512, 256), ClipRect { x1: 0, y1: 0, x2: 511, y2: 254 });
    let Some(files) = game() else { return };
    let cm01 = MapJson::from_map(&files.read_map("cm01.map").unwrap(), "cm01.map");
    assert!(cm01.clip_rect.is_none() && !cm01.to_json().contains("clipRect"));
    assert_eq!(cm01.header.name, "cm01");
    let nw = MapJson::from_map(&files.read_map("newworld.map").unwrap(), "newworld.map");
    assert_eq!(nw.clip_rect, Some(ClipRect { x1: -1, y1: 2, x2: i32::MAX, y2: 254 }));
    assert!(nw.to_json().contains("\"clipRect\": {\r\n    \"x1\": -1,"));
}

fn small() -> String {
    let row = |v: &str| format!("[ {} ]", vec![v; 32].join(", "));
    let rows = |v: &str| vec![row(v); 2].join(",\n");
    format!(
        r#"{{ "header": {{ "width": 32, "height": 2, "map": "x.map", "name": "x", "author": "me", "notes": "a \"b\"", "new": 1 }},
  "tiles": [ {} ], "cellTypes": [ {} ],
  "tileset": {{ "sources": [ {{ "filename": "well0001", "numTiles": 2 }} ],
    "tileMappings": [ {{ "tilesetIndex": 0, "tileGraphicIndex": 1 }} ], "terrainTypes": [] }},
  "tileGroups": [ {{ "name": "kept" }} ] }}"#,
        rows("0"),
        rows("7")
    )
}

#[test]
fn a_map_from_json_alone() {
    let json = MapJson::parse(&small()).unwrap();
    assert_eq!((json.header.notes.as_str(), json.extra["tileGroups"][0]["name"].as_str()), ("a \"b\"", Some("kept")));
    let map = json.to_map(None).unwrap();
    assert_eq!((map.version, map.tilesets.len(), map.clip_rect), (DEFAULT_VERSION, 512, [32, 0, 63, 0]));
    assert_eq!(map.tile(5, 1).mapping_index, 0);
    assert_eq!(map.tile_bits(5, 1) & 0x1F, 7);
    let parsed = Map::parse(&map.write()).unwrap();
    assert_eq!(parsed.tileset_name(0), Some("well0001"));
    // Unknown members are written back, Newtonsoft style.
    let text = json.to_json();
    assert!(text.contains("\"new\": 1") && text.contains("\"tileGroups\": [\r\n    {\r\n      \"name\": \"kept\""));
    assert!(text.contains("\"terrainTypes\": []"));
    assert!(text.contains("  \"tiles\": [\r\n    [    0,    0,") && text.contains("    [  7,  7,"));
}

#[test]
fn bad_maps_say_what_is_wrong() {
    let err = |text: &str| MapJson::parse(text).unwrap_err().to_string();
    let flat = small().replacen("\"tiles\": [ [", "\"tiles\": [ 0, [", 1);
    assert!(err(&flat).contains("Original format"), "{}", err(&flat));
    let short = small().replacen("\"height\": 2", "\"height\": 3", 1);
    assert!(err(&short).contains("has 2 rows"), "{}", err(&short));
    let big = small().replacen("[ 7,", "[ 40,", 1);
    assert!(err(&big).contains("most a .map can hold is 31"), "{}", err(&big));
    let wide = MapJson::parse(&small().replace("\"width\": 32", "\"width\": 33").replace(", 0 ]", ", 0, 0 ]").replace(", 7 ]", ", 7, 7 ]")).unwrap();
    assert!(wide.to_map(None).unwrap_err().to_string().contains("power of two"));
}

#[test]
fn terrain_types_encode_back_to_their_bytes() {
    let bytes: Vec<u8> = (0..TERRAIN_TYPE_BYTES as u32).map(|i| (i * 7 % 251) as u8).collect();
    let t = TerrainType::decode(&bytes);
    assert_eq!(t.encode(), bytes);
}

/// Every shipped map's tile groups as tile group JSON: the counts `docs-community/tile-group-json.md` gives, each
/// group back to the same `.map` group through its text, every map with groups written back to its bytes with them,
/// and `cm01.map`'s `snowroc9` written exactly as OP2Mapper3 writes it.
#[test]
fn tile_groups_of_the_shipped_maps() {
    let Some(files) = game() else { return };
    let (mut with_groups, mut text_named, mut groups) = (0, 0, 0);
    for name in files.map_names() {
        let (data, _) = files.read(&name).unwrap();
        let map = Map::parse(&data).unwrap();
        if map.tile_groups.is_empty() {
            continue;
        }
        with_groups += 1;
        assert_eq!(tileset_prefix(&map).as_deref(), Some("well00"), "{name}");
        let json = map_tile_groups(&map).unwrap();
        assert_eq!(json.len(), 218, "{name}");
        groups += json.len();
        let named = map.tile_groups.iter().all(|g| !g.name_bytes.is_empty() && g.name_bytes.iter().all(u8::is_ascii_graphic));
        for (g, j) in map.tile_groups.iter().zip(&json) {
            let again = TileGroupJson::parse(&j.to_json()).unwrap();
            assert_eq!(&again, j, "{name} {}", g.name);
            let back = again.to_map_group();
            assert_eq!((back.width, back.height, &back.mapping_indices), (g.width, g.height, &g.mapping_indices));
            assert!(again.max_mapping_index() <= 2011, "{name} {}: the default table's indices", g.name);
        }
        if named {
            text_named += 1;
            let mut rebuilt = map.clone();
            rebuilt.tile_groups = json.iter().map(TileGroupJson::to_map_group).collect();
            assert!(rebuilt.write() == data, "{name}: written back with groups from JSON");
        }
    }
    assert_eq!((with_groups, text_named, groups), (87, 76, 87 * 218));

    let map = files.read_map("cm01.map").unwrap();
    let snowroc9 = map_tile_groups(&map).unwrap().into_iter().find(|g| g.name == "snowroc9").unwrap();
    let expected = "{\r\n  \"header\": {\r\n    \"width\": 2,\r\n    \"height\": 3,\r\n    \"tileset\": \"well00\",\r\n    \"name\": \"snowroc9\",\r\n    \"bmp\": \"\",\r\n    \"notes\": \"\"\r\n  },\r\n  \"tiles\": [\r\n    [ 1934, 1935 ],\r\n    [ 1936, 1937 ],\r\n    [ 1938, 1939 ]\r\n  ]\r\n}";
    same_text(&snowroc9.to_json(), expected, "snowroc9");
}
