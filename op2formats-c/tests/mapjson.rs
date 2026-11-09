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
