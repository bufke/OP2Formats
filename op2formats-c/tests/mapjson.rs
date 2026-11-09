//! The JSON map functions, called the way C does, on the game's maps (`$OP2_DIR`).

mod common;

use std::mem::MaybeUninit;
use std::ptr::null;

use common::*;
use op2formats::*;

fn out<T>(f: impl FnOnce(*mut T) -> Op2fStatus) -> T {
    let mut v = MaybeUninit::<T>::uninit();
    ok(f(v.as_mut_ptr()));
    unsafe { v.assume_init() }
}

#[test]
fn maps_convert_to_json_and_back() {
    let Some(game) = game() else { return };
    let name = c("cm01.map");
    let map = out(|o| unsafe { op2f_map_open(game, name.as_ptr(), o) });
    let json = out(|o| unsafe { op2f_mapjson_from_map(map, name.as_ptr(), o) });
    let info = out(|o| unsafe { op2f_mapjson_info(json, o) });
    assert_eq!((info.width, info.height, info.source_count, info.has_clip_rect), (128, 128, 13, false));
    assert_eq!(info.clip_rect, [32, 0, 159, 126]);
    assert_eq!(string(|b, c, l| unsafe { op2f_mapjson_text(json, Op2fMapJsonText::Name, b, c, l) }), "cm01");
    ok(unsafe { op2f_mapjson_set_text(json, Op2fMapJsonText::Author, c("Dynamix").as_ptr()) });

    // Text out and back in; then to a map with the original as the base: the same .map bytes.
    let text = bytes(out(|o| unsafe { op2f_mapjson_write(json, o) }));
    assert!(String::from_utf8_lossy(&text).contains("\"author\": \"Dynamix\""));
    let again = out(|o| unsafe { op2f_mapjson_parse(text.as_ptr(), text.len(), o) });
    let rebuilt = out(|o| unsafe { op2f_mapjson_to_map(again, map, o) });
    let original = bytes(out(|o| unsafe { op2f_map_write(map, o) }));
    assert!(bytes(out(|o| unsafe { op2f_map_write(rebuilt, o) })) == original);

    // Without a base, a map the other map functions read: tiles and graphics match.
    let bare = out(|o| unsafe { op2f_mapjson_to_map(again, null(), o) });
    for (x, y) in [(0, 0), (64, 64), (127, 127)] {
        let a = out(|o| unsafe { op2f_map_tile(map, x, y, o) });
        let b = out(|o| unsafe { op2f_map_tile(bare, x, y, o) });
        assert_eq!((a.mapping_index, a.cell_type), (b.mapping_index, b.cell_type));
    }
    assert_eq!(unsafe { op2f_map_tileset_count(bare) }, 512);
    unsafe { (op2f_map_free(bare), op2f_map_free(rebuilt), op2f_mapjson_free(again), op2f_mapjson_free(json)) };
    unsafe { (op2f_map_free(map), op2f_game_free(game)) };
}

#[test]
fn bad_json_fails_cleanly() {
    let flat = br#"{ "header": { "width": 32, "height": 1 }, "tiles": [ 0, 0 ] }"#;
    assert_eq!(unsafe { op2f_mapjson_parse(flat.as_ptr(), flat.len(), &mut std::ptr::null_mut()) }, Op2fStatus::InvalidData);
    assert!(last_error().contains("Original format"), "{}", last_error());
    assert_eq!(unsafe { op2f_mapjson_open(c("/no/such/map.json").as_ptr(), &mut std::ptr::null_mut()) }, Op2fStatus::NotFound);
}

#[test]
fn tile_groups_as_json() {
    let Some(game) = game() else { return };
    let map = out(|o| unsafe { op2f_map_open(game, c("cm01.map").as_ptr(), o) });
    let count = unsafe { op2f_map_tile_group_count(map) };
    assert_eq!(count, 218);

    let group_name = |i| {
        let mut info = MaybeUninit::<Op2fTileGroupInfo>::uninit();
        string(|b, cap, l| unsafe { op2f_map_tile_group(map, i, info.as_mut_ptr(), b, cap, l) })
    };

    // cm01's snowroc9, written exactly as OP2Mapper3 writes it.
    let index = (0..count).find(|&i| group_name(i) == "snowroc9").unwrap();
    let group = out(|o| unsafe { op2f_tilegroup_from_map(map, index, o) });
    let text = String::from_utf8(bytes(out(|o| unsafe { op2f_tilegroup_write(group, o) }))).unwrap();
    let expected = "{\r\n  \"header\": {\r\n    \"width\": 2,\r\n    \"height\": 3,\r\n    \"tileset\": \"well00\",\r\n    \"name\": \"snowroc9\",\r\n    \"bmp\": \"\",\r\n    \"notes\": \"\"\r\n  },\r\n  \"tiles\": [\r\n    [ 1934, 1935 ],\r\n    [ 1936, 1937 ],\r\n    [ 1938, 1939 ]\r\n  ]\r\n}";
    assert_eq!(text, expected);
    assert_eq!(string(|b, cap, l| unsafe { op2f_tilegroup_text(group, Op2fTileGroupText::Tileset, b, cap, l) }), "well00");

    // Parsed back: the same size and tiles as the map's group.
    let parsed = out(|o| unsafe { op2f_tilegroup_parse(text.as_ptr(), text.len(), o) });
    let info = out(|o| unsafe { op2f_tilegroup_info(parsed, o) });
    assert_eq!((info.width, info.height), (2, 3));
    let mut tiles = [0u16; 6];
    ok(unsafe { op2f_tilegroup_tiles(parsed, tiles.as_mut_ptr(), tiles.len()) });
    let mut from_map = [0u32; 6];
    ok(unsafe { op2f_map_tile_group_mappings(map, index, from_map.as_mut_ptr(), from_map.len()) });
    assert_eq!(tiles.map(u32::from), from_map);
    assert_eq!(unsafe { op2f_tilegroup_tiles(parsed, tiles.as_mut_ptr(), 5) }, Op2fStatus::BufferTooSmall);

    // A new group, edited, added to the map and removed again.
    let made = out(|o| unsafe { op2f_tilegroup_new(3, 1, o) });
    ok(unsafe { op2f_tilegroup_set_tiles(made, 3, 1, [409u16, 410, 411].as_ptr()) });
    ok(unsafe { op2f_tilegroup_set_text(made, Op2fTileGroupText::Name, c("mybits").as_ptr()) });
    assert_eq!(unsafe { op2f_tilegroup_set_tiles(made, 1, 1, [2048u16].as_ptr()) }, Op2fStatus::OutOfRange);
    ok(unsafe { op2f_map_add_tile_group(map, made) });
    assert_eq!(unsafe { op2f_map_tile_group_count(map) }, 219);
    assert_eq!(group_name(218), "mybits");
    ok(unsafe { op2f_map_remove_tile_group(map, 218) });
    assert_eq!(unsafe { op2f_map_tile_group_count(map) }, 218);
    assert_eq!(unsafe { op2f_map_remove_tile_group(map, 218) }, Op2fStatus::OutOfRange);

    let mut bad = std::ptr::null_mut();
    assert_eq!(unsafe { op2f_tilegroup_parse(b"{}".as_ptr(), 2, &mut bad) }, Op2fStatus::InvalidData);
    unsafe { (op2f_tilegroup_free(made), op2f_tilegroup_free(parsed), op2f_tilegroup_free(group), op2f_map_free(map), op2f_game_free(game)) };
}
