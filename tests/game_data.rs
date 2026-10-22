//! Checks against a real game install. Skipped (passing) when no install is found, since the
//! game data can't be committed.

use op2_formats::render::MapTilesets;
use op2_formats::{GameFiles, Map, Tileset};

use op2_formats::testdata;

/// A folder of OPU's copies of the stock files (`base/<sub>`), in whichever OPU folder the game uses: its own, or the
/// one op2-rs installed. Matched case-insensitively, as GameFiles does.
fn opu_base(files: &GameFiles, sub: &str) -> std::path::PathBuf {
    let opu = op2_formats::install::opu_dir(files.root()).expect("an OPU folder (op2 setup --opu)");
    let rel = std::path::Path::new("base").join(sub);
    op2_formats::install::resolve_ci(&opu, &rel).unwrap_or_else(|| opu.join(rel))
}

fn game() -> Option<GameFiles> {
    testdata::game_files()
}

#[test]
fn every_map_parses_and_resolves_all_tiles() {
    let Some(files) = game() else { return };
    let names = files.map_names();
    assert!(names.len() > 20, "expected stock + OPU maps, found {}", names.len());

    for name in names {
        let (data, source) = files.read(&name).unwrap();
        let map = Map::parse(&data).unwrap_or_else(|e| panic!("{source}: {e}"));
        let tilesets = MapTilesets::load(&files, &map);
        for y in 0..map.height {
            for x in 0..map.width {
                assert!(tilesets.tile_pixels(&map, x, y).is_some(), "{name}: tile ({x},{y}) has no graphic");
            }
        }
    }
}

/// Stock tilesets live in maps.vol in the game's PBMP format; OPU ships loose BMP copies.
/// Both encodings should decode to identical pixels.
#[test]
fn vol_tilesets_match_loose_copies() {
    let Some(files) = game() else { return };
    let (_, maps_vol) = files.vols().find(|(n, _)| n.eq_ignore_ascii_case("maps.vol")).expect("maps.vol");

    let mut compared = 0;
    for entry in maps_vol.entries().iter().filter(|e| e.name.to_ascii_lowercase().starts_with("well")) {
        let packed = Tileset::parse(&maps_vol.read(entry).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", entry.name));
        let loose_path = opu_base(&files, "tilesets").join(entry.name.to_ascii_lowercase());
        let Ok(loose_data) = std::fs::read(&loose_path) else { continue };
        let loose = Tileset::parse(&loose_data).unwrap();
        assert_eq!(packed.tile_count, loose.tile_count, "{}", entry.name);
        assert!(packed.rgba() == loose.rgba(), "{}: pixels differ between vol and loose copy", entry.name);
        compared += 1;
    }
    assert!(compared > 5, "compared only {compared} tilesets");
}

#[test]
fn every_tile_group_resolves() {
    let Some(files) = game() else { return };
    let mut groups = 0;
    for name in files.map_names() {
        let map = Map::parse(&files.read(&name).unwrap().0).unwrap();
        let tilesets = MapTilesets::load(&files, &map);
        for group in &map.tile_groups {
            assert_eq!(group.mapping_indices.len(), (group.width * group.height) as usize, "{name}: {}", group.name);
            let (image, missing) = op2_formats::render::tile_group(&map, &tilesets, group);
            assert_eq!(missing, 0, "{name}: group {:?} has unresolved cells", group.name);
            assert_eq!(image.rgba.len(), (group.width * group.height * 32 * 32 * 4) as usize);
            groups += 1;
        }
    }
    assert!(groups > 100, "only {groups} tile groups found");
}

#[test]
fn every_sprite_image_decodes_and_unit_tables_resolve() {
    let Some(files) = game() else { return };
    let sprites = op2_formats::Sprites::load(&files).expect("load sprites");
    let art = &sprites.art;
    assert!(art.animations.len() > 2000 && art.images.len() > 5000);
    for i in 0..art.images.len() {
        sprites.image_indices(i).unwrap_or_else(|e| panic!("image {i}: {e}"));
    }

    use op2_formats::units;
    let n = art.animations.len();
    let check = |a: u16, what: &str| assert!(a == units::NONE || (a as usize) < n, "{what}: {a}");
    for v in units::vehicles() {
        for (i, row) in v.eden.iter().chain(v.plymouth).enumerate() {
            row.iter().for_each(|&a| check(a, &format!("{} row {i}", v.code)));
        }
        for col in 0..16 {
            for plymouth in [false, true] {
                let anim = v.chassis(plymouth, 0, col).unwrap_or_else(|| panic!("{} col {col}", v.code));
                let s = sprites.frame_rgba_aligned(anim, 0, op2_formats::Tint::Player(1)).unwrap();
                let opaque = s.image.rgba.chunks(4).filter(|p| p[3] == 255).count();
                assert!(opaque > 40, "{} col {col}: only {opaque} opaque pixels", v.code);
                let (hx, hy) = s.hotspot;
                assert!(hx >= 0 && hy >= 0 && (hx as u32) < s.image.width && (hy as u32) < s.image.height);
            }
        }
    }
    for b in units::buildings() {
        b.eden.iter().chain(&b.plymouth).for_each(|&a| check(a, b.code));
    }
    for t in units::turrets() {
        for set in [t.lynx, t.panther, t.tiger, t.guard_post].into_iter().flatten() {
            set.a.iter().chain(&set.b).for_each(|&a| check(a, t.code));
        }
    }
    // Lynx faces east in column 0 (hull wider than tall) and north in column 12 (taller than wide).
    let lynx = units::vehicle("LYNX").unwrap();
    let hull = |col| {
        let anim = &art.animations[lynx.chassis(false, 0, col).unwrap()];
        let layer = anim.frames[0].layers.iter().find(|l| !art.images[l.image as usize].is_shadow()).unwrap();
        let m = &art.images[layer.image as usize];
        (m.width, m.height)
    };
    let (east, north) = (hull(0), hull(12));
    assert!(east.0 > east.1 && north.1 > north.0, "east {east:?} north {north:?}");
}

/// sheets.vol is LZH-compressed. OPU ships edited loose copies of the same files, so the
/// decoded text should agree with them for most of its length (OPU edits a few fields, and
/// the stock streams end in a little garbage, which OPU's copies imitate).
#[test]
fn lzh_entries_decompress_and_match_opu_copies() {
    let Some(files) = game() else { return };
    let mut checked = 0;
    for (archive, vol) in files.vols() {
        for entry in vol.entries().iter().filter(|e| e.compression == op2_formats::vol::Compression::Lzh) {
            let data = vol.data(entry).unwrap_or_else(|e| panic!("{archive}:{}: {e}", entry.name));
            assert!(data.len() > entry.size as usize, "{}: decoded smaller than stored", entry.name);
            let lower = entry.name.to_ascii_lowercase();
            let dir = if lower.ends_with("tek.txt") { "techs" } else { "sheets" };
            let Ok(loose) = std::fs::read(opu_base(&files, dir).join(&lower)) else { continue };
            // Compare the real content, up to OPU's "; Garbage so checksum matches" trailer.
            let end = loose.windows(9).position(|w| w == b"; Garbage").unwrap_or(loose.len());
            let same = data.iter().zip(&loose[..end]).filter(|(a, b)| a == b).count();
            assert!(same * 100 > end * 98, "{}: only {same} of {end} bytes agree", entry.name);
            checked += 1;
        }
    }
    assert_eq!(checked, 10, "expected the 10 compressed sheets.vol entries");
}

#[test]
fn rules_and_tech_trees_parse() {
    use op2_formats::sheets::{self, Rules, Track};
    let Some(files) = game() else { return };
    let rules = Rules::load(&files).expect("rules");
    assert_eq!(rules.vehicles.len(), 17);
    assert!(rules.weapons.len() > 25 && rules.buildings.len() > 35 && rules.space.len() > 20);
    assert_eq!(rules.mines.len(), 9);
    assert_eq!(rules.morale["UTOPIA"][0], "90");
    let lynx = rules.vehicle("LIGHT_TANK").unwrap();
    assert_eq!((lynx.hit_points, lynx.move_speed, lynx.turn_rate, lynx.track), (300, 3, 16, Track::Wheeled));
    assert_eq!(rules.building("COMMAND").unwrap().width, 3);

    for file in sheets::TECH_FILES {
        let (text, _) = sheets::read_text(&files, file).unwrap();
        let techs = sheets::parse_techs(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(techs.len() > 20, "{file}: {} topics", techs.len());
        let ids: std::collections::HashSet<u32> = techs.iter().map(|t| t.id).collect();
        for t in &techs {
            for r in &t.requires {
                assert!(ids.contains(r), "{file}: {} requires unknown {r}", t.id);
            }
            for (code, _, _) in &t.unit_props {
                let known = rules.vehicle(code).is_some()
                    || rules.weapon(code).is_some()
                    || rules.building(code).is_some()
                    || rules.space.iter().any(|s| s.code == *code);
                assert!(known, "{file}: {} changes unknown unit {code}", t.id);
            }
        }
    }

    // Cross-check against stats the harness read from the running game, when a dump exists.
    let runs = testdata::oracle_runs_dir();
    let dump = std::fs::read_dir(&runs).ok().and_then(|mut d| d.find_map(|e| {
        std::fs::read_to_string(e.ok()?.path().join("gamedata.json")).ok()
    }));
    let Some(dump) = dump else {
        testdata::skip(&format!("no gamedata.json in {}", runs.display()));
        return;
    };
    let mut checked = 0;
    // Minimal scan of the "unitTypes" objects, avoiding a JSON dependency.
    for obj in dump.split('{').filter(|o| o.contains("\"moveSpeed\"")) {
        let field = |k: &str| -> Option<&str> {
            let at = obj.find(&format!("\"{k}\":"))? + k.len() + 3;
            Some(obj[at..].split([',', '}']).next()?.trim().trim_matches('"'))
        };
        let (Some(code), Some(hp)) = (field("code"), field("hp")) else { continue };
        let Some(v) = rules.vehicle(code) else { continue };
        if code == "SCAT" {
            continue; // not a ground vehicle; its stats don't come from this row alone
        }
        assert_eq!(hp, v.hit_points.to_string(), "{code} hp");
        assert_eq!(field("moveSpeed").unwrap(), v.move_speed.to_string(), "{code} moveSpeed");
        assert_eq!(field("turnRate").unwrap(), v.turn_rate.to_string(), "{code} turnRate");
        assert_eq!(field("armor").unwrap(), v.armor().to_string(), "{code} armor");
        assert_eq!(field("track").unwrap(), (v.track as u8).to_string(), "{code} track");
        checked += 1;
    }
    eprintln!("cross-checked {checked} vehicles against gamedata.json");
    assert!(checked >= 14, "only {checked} vehicles cross-checked against gamedata.json");
}

/// Every map rewrites to identical bytes, and edits survive a write/parse cycle.
#[test]
fn every_map_round_trips_byte_for_byte() {
    let Some(files) = game() else { return };
    let mut n = 0;
    for name in files.map_names() {
        let (data, source) = files.read(&name).unwrap();
        let mut map = Map::parse(&data).unwrap();
        let written = map.write();
        let first = written.iter().zip(&data).position(|(a, b)| a != b);
        assert!(written == data, "{source}: rewrite differs ({} vs {} bytes, first at {first:?})", written.len(), data.len());
        let bits = map.tile_bits(1, 2) ^ (5 << 5);
        map.set_tile_bits(1, 2, bits);
        let reparsed = Map::parse(&map.write()).unwrap();
        assert_eq!(reparsed.tile_bits(1, 2), bits);
        n += 1;
    }
    assert!(n >= 89, "only {n} maps");
}

#[test]
fn music_and_sound_effects_parse() {
    use op2_formats::sound::{Clm, wav_info};
    let Some(files) = game() else { return };
    let clm = Clm::parse(files.read("op2.clm").unwrap().0).unwrap();
    assert_eq!(clm.entries.len(), 26);
    assert_eq!((clm.format.samples_per_sec, clm.format.bits_per_sample), (22050, 16));
    let wav = clm.wav(&clm.entries[0]);
    assert_eq!(wav_info(&wav).unwrap().1, clm.entries[0].length as usize);
    let mut n = 0;
    for (archive, vol) in files.vols() {
        for e in vol.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".wav")) {
            let (f, len) = wav_info(&vol.data(e).unwrap()).unwrap_or_else(|err| panic!("{archive}:{}: {err}", e.name));
            assert!(len > 0 && f.samples_per_sec > 0, "{}", e.name);
            n += 1;
        }
    }
    assert!(n > 200, "only {n} wav files in .vol archives");
}
