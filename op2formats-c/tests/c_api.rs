//! The C interface called the way C code calls it: raw pointers, status codes, caller buffers. Checks that need the
//! game find it through `$OP2_DIR` and skip without it (`OP2_REQUIRE_DATA=1` makes that a failure).

use std::ffi::{CStr, CString, c_char};
use std::ptr::{null, null_mut};

use op2formats::*;

fn last_error() -> String {
    let mut buf = [0 as c_char; 512];
    unsafe { op2f_last_error(buf.as_mut_ptr(), buf.len()) };
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

fn game() -> Option<*mut Op2fGame> {
    let Some(dir) = std::env::var_os("OP2_DIR").filter(|d| !d.is_empty()) else {
        assert!(std::env::var_os("OP2_REQUIRE_DATA").is_none(), "OP2_REQUIRE_DATA is set but OP2_DIR isn't");
        eprintln!("skipping: set OP2_DIR to an Outpost 2 install");
        return None;
    };
    let dir = CString::new(dir.to_str().expect("OP2_DIR is UTF-8")).unwrap();
    let mut game = null_mut();
    assert_eq!(unsafe { op2f_game_open(dir.as_ptr(), &mut game) }, Op2fStatus::Ok, "{}", last_error());
    Some(game)
}

/// Reads a string through the length query and then a buffer of exactly that size.
fn string(mut f: impl FnMut(*mut c_char, usize, *mut usize) -> Op2fStatus) -> String {
    let mut len = 0;
    assert_eq!(f(null_mut(), 0, &mut len), Op2fStatus::Ok, "{}", last_error());
    let mut buf = vec![0 as c_char; len + 1];
    assert_eq!(f(buf.as_mut_ptr(), buf.len(), &mut len), Op2fStatus::Ok, "{}", last_error());
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

#[test]
fn errors_are_status_codes_with_messages() {
    let mut game = null_mut();
    let missing = CString::new("/no/such/outpost2/folder").unwrap();
    assert_eq!(unsafe { op2f_game_open(missing.as_ptr(), &mut game) }, Op2fStatus::NotFound);
    assert!(last_error().contains("/no/such/outpost2/folder"), "{}", last_error());
    assert!(game.is_null());

    assert_eq!(unsafe { op2f_map_parse(null(), 5, &mut null_mut()) }, Op2fStatus::NullArgument);
    assert_eq!(last_error(), "data is NULL");
    assert_eq!(unsafe { op2f_map_parse(b"junk".as_ptr(), 4, &mut null_mut()) }, Op2fStatus::InvalidData);
    assert_eq!(unsafe { op2f_tileset_parse(b"junk".as_ptr(), 4, &mut null_mut()) }, Op2fStatus::InvalidData);
    assert!(last_error().contains("neither a BMP nor a PBMP"), "{}", last_error());

    // NULL handles: getters give 0, frees do nothing.
    assert_eq!(unsafe { op2f_map_width(null()) }, 0);
    assert_eq!(unsafe { op2f_game_map_count(null()) }, 0);
    unsafe { op2f_map_free(null_mut()) };
    assert_eq!(unsafe { CStr::from_ptr(op2f_version()) }.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn sheets_parse_and_strings_truncate() {
    let text = b"Code\tHit\tMove\nName\tPoints\tSpeed\nScout\t200\t3\nLynx\t400\t5\n\n;garbage";
    let mut sheet = null_mut();
    assert_eq!(unsafe { op2f_sheet_parse(text.as_ptr(), text.len(), &mut sheet) }, Op2fStatus::Ok);
    assert_eq!(unsafe { op2f_sheet_row_count(sheet) }, 2);
    assert_eq!(string(|b, c, l| unsafe { op2f_sheet_column_name(sheet, 1, b, c, l) }), "Hit Points");
    let mut row = 0;
    let lynx = CString::new("LYNX").unwrap();
    assert_eq!(unsafe { op2f_sheet_find_row(sheet, lynx.as_ptr(), &mut row) }, Op2fStatus::Ok);
    assert_eq!(string(|b, c, l| unsafe { op2f_sheet_cell(sheet, row, 1, b, c, l) }), "400");
    assert_eq!(unsafe { op2f_sheet_cell(sheet, 0, 3, null_mut(), 0, null_mut()) }, Op2fStatus::OutOfRange);

    let mut small = [0x7f as c_char; 4];
    let mut len = 0;
    let status = unsafe { op2f_sheet_cell(sheet, 0, 0, small.as_mut_ptr(), small.len(), &mut len) };
    assert_eq!((status, len), (Op2fStatus::BufferTooSmall, 5));
    assert_eq!(unsafe { CStr::from_ptr(small.as_ptr()) }.to_str().unwrap(), "Sco");
    unsafe { op2f_sheet_free(sheet) };
}

#[test]
fn game_maps_tilesets_sheets_and_vols() {
    let Some(game) = game() else { return };

    let count = unsafe { op2f_game_map_count(game) };
    let names: Vec<String> = (0..count).map(|i| string(|b, c, l| unsafe { op2f_game_map_name(game, i, b, c, l) })).collect();
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("cm01.map")), "{names:?}");

    let name = CString::new("cm01.map").unwrap();
    let mut map = null_mut();
    assert_eq!(unsafe { op2f_map_open(game, name.as_ptr(), &mut map) }, Op2fStatus::Ok, "{}", last_error());
    let (w, h) = unsafe { (op2f_map_width(map), op2f_map_height(map)) };
    assert!(w > 0 && h > 0);
    let mut words = vec![0u32; (w * h) as usize];
    assert_eq!(unsafe { op2f_map_tile_words(map, words.as_mut_ptr(), words.len()) }, Op2fStatus::Ok);
    let mut tile = std::mem::MaybeUninit::<Op2fTile>::uninit();
    assert_eq!(unsafe { op2f_map_tile(map, w - 1, h - 1, tile.as_mut_ptr()) }, Op2fStatus::Ok);
    let tile = unsafe { tile.assume_init() };
    assert_eq!(tile.bits, words[(w * h - 1) as usize], "row-major words match op2f_map_tile");
    assert_eq!(unsafe { op2f_map_tile(map, w, 0, &mut tile.clone()) }, Op2fStatus::OutOfRange);

    // Every tile resolves to a mapping, its tileset slot and a tile in that tileset.
    let mut mapping = std::mem::MaybeUninit::<Op2fTileMapping>::uninit();
    assert_eq!(unsafe { op2f_map_mapping(map, tile.mapping_index as usize, mapping.as_mut_ptr()) }, Op2fStatus::Ok);
    let mapping = unsafe { mapping.assume_init() };
    let mut tiles = 0;
    let set = string(|b, c, l| unsafe { op2f_map_tileset(map, mapping.tileset as usize, b, c, l, &mut tiles) });
    assert!(!set.is_empty() && u32::from(mapping.graphic) < tiles, "{set}: graphic {} of {tiles}", mapping.graphic);

    let set = CString::new(set).unwrap();
    let mut tileset = null_mut();
    assert_eq!(unsafe { op2f_tileset_open(game, set.as_ptr(), &mut tileset) }, Op2fStatus::Ok, "{}", last_error());
    assert_eq!(unsafe { op2f_tileset_tile_count(tileset) }, tiles);
    let mut rgba = vec![0u8; OP2F_TILE_RGBA_BYTES];
    let status = unsafe { op2f_tileset_tile_rgba(tileset, mapping.graphic.into(), rgba.as_mut_ptr(), rgba.len()) };
    assert_eq!(status, Op2fStatus::Ok);
    assert!(rgba.chunks(4).all(|p| p[3] == 255));
    unsafe { (op2f_tileset_free(tileset), op2f_map_free(map)) };

    let vehicles = CString::new("vehicles.txt").unwrap();
    let mut sheet = null_mut();
    assert_eq!(unsafe { op2f_sheet_open(game, vehicles.as_ptr(), &mut sheet) }, Op2fStatus::Ok, "{}", last_error());
    let scout = CString::new("Scout").unwrap();
    let mut row = 0;
    assert_eq!(unsafe { op2f_sheet_find_row(sheet, scout.as_ptr(), &mut row) }, Op2fStatus::Ok, "{}", last_error());
    unsafe { op2f_sheet_free(sheet) };

    // maps.vol through the archive API matches the game's lookup of the same file.
    let dir = string(|b, c, l| unsafe { op2f_game_dir(game, b, c, l) });
    let vol_path = CString::new(format!("{dir}/maps.vol")).unwrap();
    let mut vol = null_mut();
    assert_eq!(unsafe { op2f_vol_open(vol_path.as_ptr(), &mut vol) }, Op2fStatus::Ok, "{}", last_error());
    let mut index = 0;
    assert_eq!(unsafe { op2f_vol_find(vol, name.as_ptr(), &mut index) }, Op2fStatus::Ok, "{}", last_error());
    let mut bytes = null_mut();
    assert_eq!(unsafe { op2f_vol_read(vol, index, &mut bytes) }, Op2fStatus::Ok, "{}", last_error());
    let mut from_vol = null_mut();
    let status = unsafe { op2f_map_parse(op2f_bytes_data(bytes), op2f_bytes_len(bytes), &mut from_vol) };
    assert_eq!(status, Op2fStatus::Ok, "{}", last_error());
    assert!(unsafe { op2f_map_width(from_vol) } > 0);
    unsafe { (op2f_map_free(from_vol), op2f_bytes_free(bytes), op2f_vol_free(vol), op2f_game_free(game)) };
}
