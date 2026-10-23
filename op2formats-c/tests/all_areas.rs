//! Every area of the C interface beyond the basics in c_api.rs, against a real install (`$OP2_DIR`, OPU 1.4.1):
//! each check reads real game files through the C functions and compares what it can against another route to the
//! same data.

mod common;

use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ptr::null_mut;

use common::*;
use op2formats::*;

/// A `#[repr(C)]` out-parameter filled by a call that must succeed.
fn out<T>(f: impl FnOnce(*mut T) -> Op2fStatus) -> T {
    let mut v = MaybeUninit::<T>::uninit();
    ok(f(v.as_mut_ptr()));
    unsafe { v.assume_init() }
}

#[test]
fn install_checks_and_data_dirs() {
    let Some(dir) = game_dir() else { return };
    let check = out(|o| unsafe { op2f_install_check(c(&dir).as_ptr(), false, o) });
    assert!(unsafe { op2f_install_check_ok(check) });
    assert_eq!(out(|o| unsafe { op2f_install_check_release(check, o) }), Op2fRelease::Gog);
    let summary = string(|b, c, l| unsafe { op2f_install_check_text(check, Op2fInstallText::Summary, b, c, l) });
    assert!(summary.contains("OPU 1.4.1"), "{summary}");
    assert_eq!(list(out(|o| unsafe { op2f_install_check_list(check, Op2fInstallList::Missing, o) })), Vec::<String>::new());
    unsafe { op2f_install_check_free(check) };

    let search = out(|o| unsafe { op2f_install_search(o) });
    assert!(unsafe { op2f_install_search_searched_count(search) } > 0);
    unsafe { op2f_install_search_free(search) };
    string(|b, c, l| unsafe { op2f_data_dir(Op2fDataDir::Data, b, c, l) });
    let opu = string(|b, c, l| unsafe { op2f_opu_dir(self::c(&dir).as_ptr(), b, c, l) });
    assert!(opu.to_ascii_lowercase().ends_with("opu"), "{opu}");
}

#[test]
fn exe_tables() {
    let Some(game) = game() else { return };
    let exe = out(|o| unsafe { op2f_exe_open(game, o) });
    assert_eq!(unsafe { op2f_exe_image_base(exe) }, 0x400000);
    let sounds = out(|o| unsafe { op2f_exe_sound_files(exe, o) });
    let names = list(sounds);
    assert_eq!(names.len(), OP2F_SOUND_COUNT);
    assert!(names.iter().all(|n| n.to_ascii_lowercase().ends_with(".wav")));

    let mut tubes = [0u8; 16];
    ok(unsafe { op2f_exe_tube_graphics(exe, tubes.as_mut_ptr()) });
    let mut sorted = tubes;
    sorted.sort();
    assert_eq!(sorted, std::array::from_fn(|i| i as u8), "each tube graphic once");
    let mut ambient = [0u8; OP2F_AMBIENT_ROWS * 5];
    ok(unsafe { op2f_exe_ambient_sequence(exe, ambient.as_mut_ptr()) });
    assert_eq!((ambient[0], ambient[(OP2F_AMBIENT_ROWS - 1) * 5]), (0xFF, 0xFF));

    let sounds = out(|o| unsafe { op2f_exe_sound_files(exe, o) });
    unsafe { op2f_exe_free(exe) };
    let mut ms = vec![0u32; OP2F_SOUND_COUNT];
    ok(unsafe { op2f_sound_lengths(game, sounds, ms.as_mut_ptr(), ms.len()) });
    assert!(ms.iter().filter(|&&m| m > 0).count() > OP2F_SOUND_COUNT / 2, "most sounds have a length");
    unsafe { (op2f_string_list_free(sounds), op2f_game_free(game)) };
}

#[test]
fn maps_render_write_and_masks() {
    let Some(game) = game() else { return };
    let name = c("cm01.map");
    let map = out(|o| unsafe { op2f_map_open(game, name.as_ptr(), o) });
    let (w, h) = unsafe { (op2f_map_width(map), op2f_map_height(map)) };

    // Written back unchanged, the map is the file it came from.
    let file = bytes(out(|o| unsafe { op2f_game_read(game, name.as_ptr(), o) }));
    assert!(bytes(out(|o| unsafe { op2f_map_write(map, o) })) == file, "cm01 round-trips");

    let sets = out(|o| unsafe { op2f_map_tilesets_load(game, map, o) });
    let (rw, rh, _, rgba) = image(out(|o| unsafe { op2f_map_render(map, sets, 1, o) }));
    assert_eq!((rw, rh), (w, h));
    // One pixel per tile is each tile's average colour, as op2f_map_mapping_colour gives it.
    let tile = out(|o| unsafe { op2f_map_tile(map, 3, 4, o) });
    let mut colour = [0u8; 4];
    ok(unsafe { op2f_map_mapping_colour(map, sets, tile.mapping_index, colour.as_mut_ptr()) });
    let at = ((4 * w + 3) * 4) as usize;
    assert_eq!(rgba[at..at + 4], colour);
    let mut px = vec![0u8; OP2F_TILE_RGBA_BYTES];
    ok(unsafe { op2f_map_tile_rgba(sets, map, 3, 4, px.as_mut_ptr(), px.len()) });

    let palette = out(|o| unsafe { op2f_tile_palette_build(map, sets, o) });
    let layers = unsafe { op2f_tile_palette_layer_count(palette) };
    let layer = unsafe { op2f_tile_palette_layer(palette, tile.bits) };
    assert!(layers > 1 && layer != OP2F_MISSING_LAYER && layer < layers);
    let mut from_palette = vec![0u8; OP2F_TILE_RGBA_BYTES];
    ok(unsafe { op2f_tile_palette_layers(palette, layer, 1, from_palette.as_mut_ptr(), from_palette.len()) });
    assert_eq!(from_palette, px, "the palette layer is the tile's pixels");
    let (ow, oh, _, _) = image(out(|o| unsafe { op2f_tile_palette_overview(map, palette, o) }));
    assert_eq!((ow, oh), (w * OP2F_OVERVIEW_TILE, h * OP2F_OVERVIEW_TILE));
    unsafe { op2f_tile_palette_free(palette) };

    // Editing a tile word shows in the map and in what it writes.
    ok(unsafe { op2f_map_set_tile_bits(map, 0, 0, tile.bits) });
    assert_eq!(out(|o| unsafe { op2f_map_tile(map, 0, 0, o) }).bits, tile.bits);
    assert_eq!(unsafe { op2f_map_set_tile_bits(map, w, 0, 0) }, Op2fStatus::OutOfRange);
    let mut record = [0u8; OP2F_TERRAIN_TYPE_BYTES];
    if unsafe { op2f_map_terrain_type_count(map) } > 0 {
        ok(unsafe { op2f_map_terrain_type(map, 0, record.as_mut_ptr(), record.len()) });
    }
    assert_eq!(op2f_cell_type_speed(Op2fCellType::FastPassible1 as u8), Op2fSpeed::Fast);
    unsafe { (op2f_map_tilesets_free(sets), op2f_map_free(map)) };

    // Tile groups: some map has them; each draws at 32 px per cell with its mappings resolved.
    let count = unsafe { op2f_game_map_count(game) };
    let mut groups_seen = 0;
    for i in 0..count {
        let name = c(&string(|b, c, l| unsafe { op2f_game_map_name(game, i, b, c, l) }));
        let map = out(|o| unsafe { op2f_map_open(game, name.as_ptr(), o) });
        let groups = unsafe { op2f_map_tile_group_count(map) };
        if groups > 0 {
            let sets = out(|o| unsafe { op2f_map_tilesets_load(game, map, o) });
            let mut info = Op2fTileGroupInfo { width: 0, height: 0 };
            string(|b, c, l| unsafe { op2f_map_tile_group(map, 0, &mut info, b, c, l) });
            let mut mappings = vec![0u32; (info.width * info.height) as usize];
            ok(unsafe { op2f_map_tile_group_mappings(map, 0, mappings.as_mut_ptr(), mappings.len()) });
            let mut missing = usize::MAX;
            let (gw, gh, _, _) = image(out(|o| unsafe { op2f_map_render_tile_group(map, sets, 0, o, &mut missing) }));
            assert_eq!((gw, gh), (info.width * 32, info.height * 32));
            assert_eq!(missing, 0);
            unsafe { op2f_map_tilesets_free(sets) };
            groups_seen += groups;
        }
        unsafe { op2f_map_free(map) };
        if groups_seen > 0 {
            break;
        }
    }
    assert!(groups_seen > 0, "no map has tile groups");

    let ts = out(|o| unsafe { op2f_tileset_open(game, c("well0001").as_ptr(), o) });
    let (tw, th, _, _) = image(out(|o| unsafe { op2f_tileset_image(ts, o) }));
    assert_eq!((tw, th), (32, 32 * unsafe { op2f_tileset_tile_count(ts) }));
    unsafe { op2f_tileset_free(ts) };

    let raw = bytes(out(|o| unsafe { op2f_game_read(game, c("virmask.raw").as_ptr(), o) }));
    let mut masks = [0u32; 512];
    ok(unsafe { op2f_virmask_parse(raw.as_ptr(), raw.len(), masks.as_mut_ptr()) });
    assert_eq!(masks[..32], [0; 32], "mask 0 (no Blight around) is empty");
    let expected = op2_formats::virmask::parse(&raw).unwrap();
    assert_eq!(masks[..], *expected.as_flattened(), "the same masks as op2-formats decodes");
    unsafe { op2f_game_free(game) };
}

#[test]
fn rules_techs_and_strings() {
    let Some(game) = game() else { return };
    let rules = out(|o| unsafe { op2f_rules_load(game, o) });
    let lynx = out(|o| unsafe { op2f_rules_find(rules, Op2fRulesTable::Vehicles, c("light_tank").as_ptr(), o) });
    let stats = out(|o| unsafe { op2f_rules_vehicle(rules, lynx, o) });
    assert_eq!(field(&stats.code), "LIGHT_TANK");
    assert_eq!(stats.armor, op2f_armor_value(stats.armor_class));

    // The typed row agrees with the plain sheet.
    let sheet = out(|o| unsafe { op2f_sheet_open(game, c("vehicles.txt").as_ptr(), o) });
    let row = out(|o| unsafe { op2f_sheet_find_row(sheet, c("LIGHT_TANK").as_ptr(), o) });
    assert_eq!(row, lynx);
    let hp = string(|b, c, l| unsafe { op2f_sheet_cell(sheet, row, 4, b, c, l) });
    assert_eq!(hp.parse::<i32>().unwrap(), stats.hit_points);
    unsafe { op2f_sheet_free(sheet) };

    for (table, n) in [(Op2fRulesTable::Weapons, 0), (Op2fRulesTable::Buildings, 0), (Op2fRulesTable::Space, 0), (Op2fRulesTable::Mines, 0)] {
        assert!(unsafe { op2f_rules_count(rules, table) } > n, "{table:?}");
    }
    let cc = out(|o| unsafe { op2f_rules_building(rules, 0, o) });
    assert_eq!(field(&cc.code), "COMMAND");
    out(|o| unsafe { op2f_rules_weapon(rules, 0, o) });
    out(|o| unsafe { op2f_rules_space(rules, 0, o) });
    out(|o| unsafe { op2f_rules_mine(rules, 0, o) });
    let morale = out(|o| unsafe { op2f_rules_morale(rules, o) });
    assert!(unsafe { op2f_sheet_row_count(morale) } > 0);
    assert_eq!(unsafe { op2f_sheet_column_count(morale) }, 0);
    unsafe { op2f_sheet_free(morale) };
    let sources = out(|o| unsafe { op2f_rules_sources(rules, o) });
    assert_eq!(unsafe { op2f_string_pairs_count(sources) }, 6);
    unsafe { (op2f_string_pairs_free(sources), op2f_rules_free(rules)) };

    // Vehicle row r is named by string OP2F_VEHICLE_NAMES + r: the Lynx.
    let strings = out(|o| unsafe { op2f_game_strings_load(game, o) });
    let name = string(|b, c, l| unsafe { op2f_string_table_find(strings, OP2F_VEHICLE_NAMES + lynx as u32, b, c, l) });
    assert_eq!(name, "Lynx");
    unsafe { op2f_string_table_free(strings) };

    assert_eq!(op2f_tech_file_count(), 4);
    let file = unsafe { CStr::from_ptr(op2f_tech_file_name(0)) };
    assert_eq!(file.to_str().unwrap(), "edentek.txt");
    assert!(op2f_tech_file_name(4).is_null());
    let tree = out(|o| unsafe { op2f_tech_tree_open(game, file.as_ptr(), o) });
    let count = unsafe { op2f_tech_count(tree) };
    assert!(count > 20, "{count} techs");
    let mut requiring = 0;
    for i in 0..count {
        let info = out(|o| unsafe { op2f_tech_info(tree, i, o) });
        assert_eq!(out(|o| unsafe { op2f_tech_find(tree, info.id, o) }), i);
        string(|b, c, l| unsafe { op2f_tech_text(tree, i, Op2fTechText::Name, b, c, l) });
        let mut req = vec![0u32; info.requires_count];
        ok(unsafe { op2f_tech_requires(tree, i, req.as_mut_ptr(), req.len()) });
        requiring += (!req.is_empty()) as usize;
        for p in 0..info.unit_prop_count {
            out(|o| unsafe { op2f_tech_unit_prop(tree, i, p, o) });
        }
    }
    assert!(requiring > 0);
    unsafe { (op2f_tech_tree_free(tree), op2f_game_free(game)) };
}

#[test]
fn sound_music_and_archives() {
    let Some(game) = game() else { return };
    let clm = out(|o| unsafe { op2f_clm_open(game, o) });
    let format = out(|o| unsafe { op2f_clm_format(clm, o) });
    assert_eq!((format.format_tag, format.bits_per_sample), (1, 16));
    let tracks = unsafe { op2f_clm_track_count(clm) };
    assert!(tracks > 5);
    let mut size = 0;
    let first = string(|b, c, l| unsafe { op2f_clm_track(clm, 0, &mut size, b, c, l) });
    assert_eq!(out(|o| unsafe { op2f_clm_find(clm, c(&first).as_ptr(), o) }), 0);
    let samples = bytes(out(|o| unsafe { op2f_clm_samples(clm, 0, o) }));
    assert_eq!(samples.len(), size as usize);
    let wav = bytes(out(|o| unsafe { op2f_clm_wav(clm, 0, o) }));
    let mut data_len = 0;
    let wf = out(|o| unsafe { op2f_wav_info(wav.as_ptr(), wav.len(), o, &mut data_len) });
    assert_eq!((data_len, wf.samples_per_sec), (samples.len(), format.samples_per_sec));
    assert_eq!(wav[wav.len() - samples.len()..], samples[..]);
    assert!(unsafe { op2f_wave_seconds(&wf, data_len) } > 1.0);
    unsafe { op2f_clm_free(clm) };

    // sheets.vol is LZH-compressed: the stored bytes decompress to what op2f_vol_read gives.
    let dir = string(|b, c, l| unsafe { op2f_game_dir(game, b, c, l) });
    let vols = list(out(|o| unsafe { op2f_game_vols(game, o) }));
    assert!(vols.iter().any(|v| v.eq_ignore_ascii_case("sheets.vol")), "{vols:?}");
    let vol = out(|o| unsafe { op2f_vol_open(c(&format!("{dir}/sheets.vol")).as_ptr(), o) });
    let info = out(|o| unsafe { op2f_vol_entry_info(vol, 0, o) });
    assert_eq!((info.compression, info.compression_code), (Op2fCompression::Lzh, 0x103));
    let stored = bytes(out(|o| unsafe { op2f_vol_read_stored(vol, 0, o) }));
    let read = bytes(out(|o| unsafe { op2f_vol_read(vol, 0, o) }));
    assert!(bytes(out(|o| unsafe { op2f_lzh_decompress(stored.as_ptr(), stored.len(), o) })) == read);
    unsafe { op2f_vol_free(vol) };

    let vol = out(|o| unsafe { op2f_vol_open(c(&format!("{dir}/sound.vol")).as_ptr(), o) });
    let wav = bytes(out(|o| unsafe { op2f_vol_read(vol, 0, o) }));
    let mut data_len = 0;
    out(|o| unsafe { op2f_wav_info(wav.as_ptr(), wav.len(), o, &mut data_len) });
    assert!(data_len > 0);
    unsafe { (op2f_vol_free(vol), op2f_game_free(game)) };
}

#[test]
fn cutscenes_decode_in_any_order() {
    let Some(game) = game() else { return };
    let name = c("CEP.avi");
    let video = out(|o| unsafe { op2f_video_open(game, name.as_ptr(), o) });
    let info = out(|o| unsafe { op2f_video_info(video, o) });
    assert_eq!((info.width, info.height), (640, 480));
    assert!(info.frame_count > 30 && info.has_audio, "{} frames", info.frame_count);
    let size = (info.width * info.height * 4) as usize;
    assert!(out(|o| unsafe { op2f_video_is_keyframe(video, 0, o) }));

    // Frame 30 straight away, then frames in order up to it on a fresh handle: the same picture.
    let mut direct = vec![0u8; size];
    ok(unsafe { op2f_video_frame(video, 30, direct.as_mut_ptr(), size) });
    let fresh = out(|o| unsafe { op2f_video_open(game, name.as_ptr(), o) });
    let mut stepped = vec![0u8; size];
    for f in 0..=30 {
        ok(unsafe { op2f_video_frame(fresh, f, stepped.as_mut_ptr(), size) });
    }
    assert!(direct == stepped, "frame 30 decodes the same either way");
    // And back to frame 0 after it.
    let mut first = vec![0u8; size];
    ok(unsafe { op2f_video_frame(fresh, 0, first.as_mut_ptr(), size) });
    assert!(first.chunks(4).all(|p| p[3] == 255));
    assert_eq!(unsafe { op2f_video_frame(fresh, info.frame_count as usize, first.as_mut_ptr(), size) }, Op2fStatus::OutOfRange);

    let pcm = bytes(out(|o| unsafe { op2f_video_audio(video, o) }));
    let seconds = pcm.len() as f64 / (2.0 * info.audio.channels as f64 * info.audio.samples_per_sec as f64);
    let film = info.frame_count as f64 * info.micros_per_frame as f64 / 1e6;
    assert!((seconds - film).abs() < 2.0, "{seconds:.1} s of audio for {film:.1} s of video");
    assert!(unsafe { op2f_video_stream_count(video) } >= 2);
    let stream = out(|o| unsafe { op2f_video_stream_info(video, 0, o) });
    assert!(stream.chunk_count > 0);
    bytes(out(|o| unsafe { op2f_video_chunk(video, 0, 0, o) }));
    unsafe { (op2f_video_free(fresh), op2f_video_free(video), op2f_game_free(game)) };
}

#[test]
fn windows_resources_and_cursors() {
    let Some(game) = game() else { return };
    let res = out(|o| unsafe { op2f_resources_open(game, c("out2res.dll").as_ptr(), o) });
    let count = unsafe { op2f_resources_count(res) };
    assert!(count > 50, "{count} resources");
    let (mut bitmaps, mut cursors) = (0, 0);
    for i in 0..count {
        let info = out(|o| unsafe { op2f_resource_info(res, i, o) });
        let name = string(|b, c, l| unsafe { op2f_resource_name(res, i, b, c, l) });
        let kind = string(|b, c, l| unsafe { op2f_resource_type(res, i, b, c, l) });
        assert_eq!(out(|o| unsafe { op2f_resources_find(res, c(&kind).as_ptr(), c(&name).as_ptr(), o) }), i, "{kind} {name}");
        if info.type_id == 2 && bitmaps < 5 {
            let bmp = out(|o| unsafe { op2f_resources_bitmap(res, c(&name).as_ptr(), o) });
            let bi = out(|o| unsafe { op2f_bitmap_info(bmp, o) });
            let (w, h, _, _) = image(out(|o| unsafe { op2f_bitmap_image(bmp, o) }));
            assert_eq!((w, h), (bi.width, bi.height));
            if bi.palette_count > 0 {
                let mut indices = vec![0u8; (w * h) as usize];
                ok(unsafe { op2f_bitmap_indices(bmp, indices.as_mut_ptr(), indices.len()) });
            }
            unsafe { op2f_bitmap_free(bmp) };
            bitmaps += 1;
        }
        if info.type_id == 12 && cursors < 3 {
            let (w, h, (hx, hy), _) = image(out(|o| unsafe { op2f_resources_cursor(res, c(&name).as_ptr(), true, o) }));
            assert!(hx >= 0 && (hx as u32) < w && hy >= 0 && (hy as u32) < h);
            cursors += 1;
        }
        bytes(out(|o| unsafe { op2f_resource_bytes(res, i, o) }));
    }
    assert!(bitmaps > 0 && cursors > 0, "{bitmaps} bitmaps, {cursors} cursors");
    let strings = out(|o| unsafe { op2f_resources_strings(res, o) });
    unsafe { op2f_string_table_free(strings) };
    unsafe { op2f_resources_free(res) };

    // The shell's dialogs, and Outpost2.exe's version.
    let shell = out(|o| unsafe { op2f_resources_open(game, c("op2shres.dll").as_ptr(), o) });
    let dialog_id = (0..unsafe { op2f_resources_count(shell) })
        .map(|i| out(|o| unsafe { op2f_resource_info(shell, i, o) }))
        .find(|i| i.type_id == 5 && i.name_id != 0)
        .expect("a dialog")
        .name_id;
    let dialog = out(|o| unsafe { op2f_resources_dialog(shell, dialog_id, o) });
    let di = out(|o| unsafe { op2f_dialog_info(dialog, o) });
    for k in 0..di.control_count {
        out(|o| unsafe { op2f_dialog_control(dialog, k, o) });
        string(|b, c, l| unsafe { op2f_dialog_control_class(dialog, k, b, c, l, null_mut(), 0) });
    }
    string(|b, c, l| unsafe { op2f_dialog_text(dialog, Op2fDialogText::Title, b, c, l) });
    unsafe { (op2f_dialog_free(dialog), op2f_resources_free(shell)) };

    let exe = out(|o| unsafe { op2f_resources_open(game, c("Outpost2.exe").as_ptr(), o) });
    let version = out(|o| unsafe { op2f_resources_version(exe, o) });
    let keys: Vec<String> = (0..unsafe { op2f_string_pairs_count(version) })
        .map(|i| string(|b, c, l| unsafe { op2f_string_pairs_key(version, i, b, c, l) }))
        .collect();
    assert!(keys.iter().any(|k| k == "FileVersion"), "{keys:?}");
    unsafe { (op2f_string_pairs_free(version), op2f_resources_free(exe)) };

    // An animated cursor from the game folder.
    let ani = bytes(out(|o| unsafe { op2f_game_read(game, c("attack.ani").as_ptr(), o) }));
    let cursor = out(|o| unsafe { op2f_ani_cursor_parse(ani.as_ptr(), ani.len(), o) });
    let (frames, steps) = unsafe { (op2f_ani_cursor_frame_count(cursor), op2f_ani_cursor_step_count(cursor)) };
    assert!(frames > 0 && steps > 0);
    for s in 0..steps {
        let mut frame = usize::MAX;
        let mut jiffies = 0;
        ok(unsafe { op2f_ani_cursor_step(cursor, s, &mut frame, &mut jiffies) });
        assert!(frame < frames);
    }
    image(out(|o| unsafe { op2f_ani_cursor_frame(cursor, 0, o) }));
    unsafe { (op2f_ani_cursor_free(cursor), op2f_game_free(game)) };
}

#[test]
fn sprite_details_unit_art_and_picking() {
    let Some(game) = game() else { return };
    let sprites = out(|o| unsafe { op2f_sprites_open(game, o) });
    assert!(unsafe { op2f_sprites_palette_count(sprites) } > 0);
    let mut palette = [0u8; 768];
    ok(unsafe { op2f_sprites_palette(sprites, 0, palette.as_mut_ptr()) });

    // A Lynx facing east: its chassis animation, frame 0's layers, and the drawn frame's pixels.
    let lynx = c("Lynx");
    let anim = out(|o| unsafe { op2f_vehicle_animation(lynx.as_ptr(), false, 0, 0, o) });
    let ainfo = out(|o| unsafe { op2f_sprites_animation_info(sprites, anim, o) });
    let finfo = out(|o| unsafe { op2f_sprites_frame_info(sprites, anim, 0, o) });
    assert!(finfo.layer_count > 0 && finfo.has_optional1, "the chassis says where its turret sits");
    let layer = out(|o| unsafe { op2f_sprites_frame_layer(sprites, anim, 0, 0, o) });
    let meta = out(|o| unsafe { op2f_sprites_image_info(sprites, layer.image as usize, o) });
    let mut indices = vec![0u8; (meta.width * meta.height) as usize];
    ok(unsafe { op2f_sprites_image_indices(sprites, layer.image as usize, indices.as_mut_ptr(), indices.len()) });
    ok(unsafe { op2f_sprites_image_palette(sprites, layer.image as usize, 1, palette.as_mut_ptr()) });
    let mut bounds = [0i32; 4];
    ok(unsafe { op2f_sprites_frame_bounds(sprites, anim, 0, bounds.as_mut_ptr()) });
    let (fw, fh, _, _) = image(out(|o| unsafe { op2f_sprites_frame(sprites, anim, 0, -1, false, o) }));
    assert_eq!((fw as i32, fh as i32), (bounds[2] - bounds[0], bounds[3] - bounds[1]));
    ok(unsafe { op2f_sprites_animation_bounds(sprites, anim, bounds.as_mut_ptr()) });
    for r in 0..ainfo.appendix_count {
        let mut rect = [0u32; 4];
        ok(unsafe { op2f_sprites_animation_appendix(sprites, anim, r, rect.as_mut_ptr()) });
    }

    // Tables of unit art, a turret by facing, and the tank drawn both ways.
    for table in [Op2fUnitArtTable::Vehicles, Op2fUnitArtTable::Buildings, Op2fUnitArtTable::Turrets] {
        let n = op2f_unit_art_count(table);
        assert!(n > 0 && !op2f_unit_art_code(table, n - 1).is_null() && op2f_unit_art_code(table, n).is_null());
    }
    assert_eq!(op2f_direction_column(op2f_column_rotation(5)), 5);
    let laser = c("LASER");
    let turret = out(|o| unsafe { op2f_turret_animation(laser.as_ptr(), lynx.as_ptr(), false, 0, o) });
    let (mut tx, mut ty, mut mx, mut my) = (0, 0, 0, 0);
    ok(unsafe { op2f_turret_offset(sprites, anim, &mut tx, &mut ty) });
    ok(unsafe { op2f_muzzle_offset(sprites, anim, turret, &mut mx, &mut my) });
    assert_eq!((tx, ty), (finfo.optional1[0] as i32, finfo.optional1[1] as i32));
    let chassis = out(|o| unsafe { op2f_sprites_vehicle(sprites, lynx.as_ptr(), false, 0, 0, 2, o) });
    let top = out(|o| unsafe { op2f_sprites_frame(sprites, turret, 0, 2, true, o) });
    let built = image(out(|o| unsafe { op2f_image_overlay(chassis, top, tx, ty, o) }));
    let tank = image(out(|o| unsafe { op2f_sprites_tank(sprites, lynx.as_ptr(), laser.as_ptr(), false, 0, 2, o) }));
    assert!(built == tank, "chassis + turret by hand is the tank");
    unsafe { (op2f_image_free(chassis), op2f_image_free(top)) };
    let mut ambient = 9;
    ok(unsafe { op2f_building_ambient(c("COMMAND").as_ptr(), false, &mut ambient) });

    // Picking: a click on the Lynx's hotspot selects it; one far off hits nothing.
    let shape = Op2fPickShape {
        kind: Op2fPickKind::Box,
        animation: anim,
        frame: 0,
        has_overlay: false,
        overlay_animation: 0,
        overlay_main_rect: false,
    };
    let list = [Op2fPickCandidate { x: 200, y: 200, owner: 0, shape }];
    assert_eq!(out(|o| unsafe { op2f_pick_select(sprites, list.as_ptr(), 1, 200, 200, 0, o) }), 0);
    assert_eq!(out(|o| unsafe { op2f_pick_target(sprites, list.as_ptr(), 1, 200, 200, o) }), 0);
    assert_eq!(unsafe { op2f_pick_select(sprites, list.as_ptr(), 1, 900, 900, 0, &mut 0) }, Op2fStatus::NotFound);
    assert!(out(|o| unsafe { op2f_pick_hits(sprites, &shape, 0, 0, o) }));
    let mut sel = [0i32; 4];
    ok(unsafe { op2f_pick_selection_box(sprites, anim, sel.as_mut_ptr()) });
    let mut mine = shape;
    ok(unsafe { op2f_pick_overlay(c("MINE").as_ptr(), 0, false, &mut mine) });
    assert!(mine.has_overlay && mine.overlay_main_rect);
    unsafe { (op2f_sprites_free(sprites), op2f_game_free(game)) };
}

#[test]
fn images_and_string_lists() {
    let pixels: Vec<u8> = (0..16 * 8).flat_map(|i| [i as u8, 0, 0, 255]).collect();
    let img = out(|o| unsafe { op2f_image_new(16, 8, pixels.as_ptr(), 4, 2, o) });
    let (w, h, hs, _) = image(out(|o| unsafe { op2f_image_shrink(img, 2, o) }));
    assert_eq!((w, h, hs), (8, 4, (2, 1)));
    let mut count = 0;
    ok(unsafe { op2f_image_mip_chain(img, null_mut(), 0, &mut count) });
    assert_eq!(count, 5, "16x8, 8x4, 4x2, 2x1, 1x1");
    let mut chain = vec![null_mut(); count];
    ok(unsafe { op2f_image_mip_chain(img, chain.as_mut_ptr(), chain.len(), &mut count) });
    assert_eq!(chain.into_iter().map(|i| image(i).0).collect::<Vec<_>>(), [16, 8, 4, 2, 1]);
    unsafe { op2f_image_free(img) };

    let text = b"[Game]\n718=\"Lynx\"\n100=\"two\\nlines\"\n";
    let table = out(|o| unsafe { op2f_game_strings_parse(text.as_ptr(), text.len(), o) });
    assert_eq!(unsafe { op2f_string_table_count(table) }, 2);
    let mut id = 0;
    let first = string(|b, c, l| unsafe { op2f_string_table_entry(table, 0, &mut id, b, c, l) });
    assert_eq!((id, first.as_str()), (100, "two\nlines"));
    assert_eq!(unsafe { op2f_string_table_find(table, 5, null_mut(), 0, null_mut()) }, Op2fStatus::NotFound);
    unsafe { op2f_string_table_free(table) };
}

#[test]
fn installers_without_installers() {
    // No installer files here: these only check the calls fail cleanly and the static facts.
    let version = unsafe { CStr::from_ptr(op2f_opu_text(Op2fOpuText::Version)) };
    assert_eq!(version.to_str().unwrap(), "1.4.1");
    assert!(op2f_opu_package_size() > 1_000_000);
    assert_eq!(unsafe { op2f_opu_verify_package(b"junk".as_ptr(), 4) }, Op2fStatus::InvalidData);
    assert_ne!(unsafe { op2f_nsis_parse(b"junk".as_ptr(), 4, &mut null_mut()) }, Op2fStatus::Ok);
    let status = unsafe { op2f_inno_open(c("/no/such/setup.exe").as_ptr(), &mut null_mut()) };
    assert!(matches!(status, Op2fStatus::NotFound | Op2fStatus::Io), "{status:?}: {}", last_error());
    let status = unsafe { op2f_opu_download(c(".").as_ptr(), None, null_mut(), null_mut(), 0, null_mut()) };
    assert_eq!(status, Op2fStatus::Unsupported, "a default build has no downloader");
}
