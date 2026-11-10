//! The `.opm` functions, called the way C does, on op2-formats' test missions.

mod common;

use std::mem::MaybeUninit;
use std::ptr::null_mut;

use common::*;
use op2formats::*;

fn out<T>(f: impl FnOnce(*mut T) -> Op2fStatus) -> T {
    let mut v = MaybeUninit::<T>::uninit();
    ok(f(v.as_mut_ptr()));
    unsafe { v.assume_init() }
}

fn fixture(name: &str) -> String {
    format!("{}/../tests/data/opm/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn reads_a_mission() {
    let opm = out(|o| unsafe { op2f_opm_open(c(&fixture("opu_12.opm")).as_ptr(), o) });
    let level = out(|o| unsafe { op2f_opm_level(opm, o) });
    assert_eq!((level.mission_type, level.num_players, level.max_tech_level, level.variant_count), (OP2F_MISSION_COLONY, 2, 12, 1));
    assert_eq!(string(|b, c, l| unsafe { op2f_opm_text(opm, Op2fOpmText::MapName, b, c, l) }), "opu_12.map");
    assert_eq!(string(|b, c, l| unsafe { op2f_opm_text(opm, Op2fOpmText::FilePrefix, b, c, l) }), "c");
    assert_eq!(unsafe { op2f_opm_variant_name(opm, 0, null_mut(), 0, null_mut()) }, Op2fStatus::NotFound);
    assert_eq!(unsafe { op2f_opm_player_count(opm, 0) }, 2);

    let p0 = out(|o| unsafe { op2f_opm_player(opm, 0, 0, o) });
    assert!(p0.is_eden && p0.is_human && p0.color == 0 && p0.ally_count == 0);
    assert_eq!(string(|b, c, l| unsafe { op2f_opm_player_text(opm, 0, 0, Op2fOpmPlayerText::BotType, b, c, l) }), "None");
    let r = out(|o| unsafe { op2f_opm_resources(opm, 0, 0, -1, o) });
    assert_eq!((r.center_x, r.center_y, r.common_ore, r.food), (17, 17, 2000, 1000));
    assert_eq!(string(|b, c, l| unsafe { op2f_opm_morale(opm, 0, 0, -1, b, c, l) }), "Good");
    let mut none = MaybeUninit::<Op2fOpmResources>::uninit();
    assert_eq!(unsafe { op2f_opm_resources(opm, 0, 0, 0, none.as_mut_ptr()) }, Op2fStatus::OutOfRange, "no difficulties");

    let cc = out(|o| unsafe { op2f_opm_unit(opm, 0, 0, -1, 0, o) });
    assert_eq!((cc.x, cc.y, cc.direction, cc.health, cc.cargo_kind), (15, 15, 0, 1.0, Op2fOpmCargo::None));
    assert_eq!(cc.bar_yield, OP2F_OPM_UNKNOWN, "a structure's BarYield is null");
    let ty = string(|b, c, l| unsafe { op2f_opm_unit_text(opm, 0, 0, -1, 0, Op2fOpmUnitText::TypeId, b, c, l) });
    assert_eq!(ty, "CommandCenter");

    // Every unit decodes; the weapons carriers say so, and the cargo trucks carry what their CargoType names.
    let (mut weapons, mut trucks) = (0, 0);
    for p in 0..2 {
        for u in 0..out(|o| unsafe { op2f_opm_resources(opm, 0, p, -1, o) }).unit_count {
            let unit = out(|o| unsafe { op2f_opm_unit(opm, 0, p, -1, u, o) });
            assert!((0..8).contains(&unit.direction));
            weapons += (unit.cargo_kind == Op2fOpmCargo::Weapon) as usize;
            trucks += (unit.cargo_kind == Op2fOpmCargo::Truck && (1..=7).contains(&unit.cargo)) as usize;
        }
    }
    assert!(weapons > 0 && trucks > 0, "{weapons} armed units, {trucks} loaded trucks");

    let game = out(|o| unsafe { op2f_opm_game(opm, 0, -1, o) });
    assert_eq!((game.marker_count, game.wreckage_count, game.song_count), (3, 1, 1));
    let (mut id, mut x, mut y) = (0, 0, 0);
    let kind = string(|b, c, l| unsafe { op2f_opm_marker(opm, 0, -1, 1, &mut id, &mut x, &mut y, b, c, l) });
    assert_eq!((kind.as_str(), id, x, y), ("DNA", 2, 40, 34));
    let w = out(|o| unsafe { op2f_opm_wreckage(opm, 0, -1, 0, o) });
    assert_eq!((w.tech_id, w.is_visible, w.x, w.y), (8000, true, 44, 34));
    for b in 0..game.beacon_count {
        let mut beacon = MaybeUninit::<Op2fOpmBeacon>::uninit();
        string(|buf, c, l| unsafe { op2f_opm_beacon(opm, 0, -1, b, beacon.as_mut_ptr(), buf, c, l) });
    }
    let mut songs = vec![0; game.song_count];
    ok(unsafe { op2f_opm_songs(opm, 0, -1, songs.as_mut_ptr(), songs.len()) });
    unsafe { op2f_opm_free(opm) };
}

#[test]
fn writes_what_it_reads() {
    for name in ["opu_12.opm", "cmyTut.opm"] {
        let opm = out(|o| unsafe { op2f_opm_open(c(&fixture(name)).as_ptr(), o) });
        let json = bytes(out(|o| unsafe { op2f_opm_json(opm, o) }));
        assert!(json.starts_with(b"\r\n{\r\n\t\"Disasters\":"));
        let again = out(|o| unsafe { op2f_opm_parse(json.as_ptr(), json.len(), o) });
        assert!(bytes(out(|o| unsafe { op2f_opm_json(again, o) })) == json, "{name}: written twice, the same");

        let path = std::env::temp_dir().join(format!("op2formats-test-{}-{name}", std::process::id()));
        ok(unsafe { op2f_opm_save(opm, c(path.to_str().unwrap()).as_ptr()) });
        let saved = out(|o| unsafe { op2f_opm_open(c(path.to_str().unwrap()).as_ptr(), o) });
        let level = out(|o| unsafe { op2f_opm_level(saved, o) });
        assert_eq!(level.num_players, out(|o| unsafe { op2f_opm_level(opm, o) }).num_players);
        std::fs::remove_file(&path).unwrap();
        unsafe { (op2f_opm_free(saved), op2f_opm_free(again), op2f_opm_free(opm)) };
    }
}

#[test]
fn bad_files_fail_cleanly() {
    assert_eq!(unsafe { op2f_opm_parse(b"{ nope".as_ptr(), 6, &mut null_mut()) }, Op2fStatus::InvalidData);
    assert!(last_error().contains("line 1"), "{}", last_error());
    assert_eq!(unsafe { op2f_opm_open(c("/no/such/mission.opm").as_ptr(), &mut null_mut()) }, Op2fStatus::NotFound);
    assert_eq!(unsafe { op2f_opm_parse([0xFFu8, 0xFE].as_ptr(), 2, &mut null_mut()) }, Op2fStatus::InvalidData);
}
