//! The mission DLL functions, called the way C does, against a real install (`$OP2_DIR`, OPU 1.4.1), compared with
//! op2-formats' own reading of the same files.

mod common;

use std::mem::MaybeUninit;
use std::ptr::{null, null_mut};

use common::*;
use op2formats::*;

fn out<T>(f: impl FnOnce(*mut T) -> Op2fStatus) -> T {
    let mut v = MaybeUninit::<T>::uninit();
    ok(f(v.as_mut_ptr()));
    unsafe { v.assume_init() }
}

#[test]
fn an_install_s_missions() {
    let Some(game) = game() else { return };
    let missions = out(|o| unsafe { op2f_game_missions(game, o) });
    let count = out(|o| unsafe { op2f_mission_list_count(missions, o) });
    assert_eq!(count, 159);

    let mut colonies = 0;
    for i in 0..count {
        let m = out(|o| unsafe { op2f_mission_list_get(missions, i, o) });
        let path = string(|b, c, l| unsafe { op2f_mission_list_path(missions, i, b, c, l) });
        let desc = out(|o| unsafe { op2f_mission_desc(m, o) });
        let text = |which| string(|b, c, l| unsafe { op2f_mission_text(m, which, b, c, l) });

        // The same file read by op2-formats directly.
        let direct = op2_formats::mission::MissionDll::open(&path).unwrap();
        assert_eq!(desc.mission_type, direct.mission_type.value(), "{path}");
        assert_eq!((desc.players, desc.max_tech_level, desc.unit_only), (direct.players, direct.max_tech_level, direct.unit_only));
        assert_eq!(desc.has_desc_block_ex, direct.extra_ai_players.is_some());
        assert_eq!(desc.checksum, direct.checksum);
        assert_eq!(text(Op2fMissionText::LevelDesc), direct.level_desc);
        assert_eq!(text(Op2fMissionText::MapName), direct.map_name);
        assert_eq!(text(Op2fMissionText::TechtreeName), direct.techtree_name);
        assert_eq!((desc.export_count, desc.warning_count), (direct.exports.len(), 0));

        let checksum = out(|o| unsafe {
            let data = std::fs::read(&path).unwrap();
            op2f_mission_checksum(data.as_ptr(), data.len(), o)
        });
        assert_eq!(checksum, desc.checksum, "{path}");
        colonies += (desc.mission_type == OP2F_MISSION_COLONY) as usize;
    }
    assert_eq!(colonies, 24);
    unsafe { op2f_mission_list_free(missions) };
    unsafe { op2f_game_free(game) };
}

#[test]
fn one_mission_s_exports() {
    let Some(dir) = game_dir() else { return };
    let path = op2_formats::find_in_dir(std::path::Path::new(&dir), "e01.dll").unwrap();
    let m = out(|o| unsafe { op2f_mission_open(c(&path.to_string_lossy()).as_ptr(), o) });
    let desc = out(|o| unsafe { op2f_mission_desc(m, o) });
    assert_eq!((desc.mission_type, desc.max_tech_level), (1, 1));
    assert_eq!(string(|b, c, l| unsafe { op2f_mission_text(m, Op2fMissionText::LevelDesc, b, c, l) }), "Eden Mission #1");

    let mut names = Vec::new();
    for i in 0..desc.export_count {
        let mut e = MaybeUninit::<Op2fMissionExport>::uninit();
        let name = string(|b, c, l| unsafe { op2f_mission_export(m, i, e.as_mut_ptr(), b, c, l) });
        let e = unsafe { e.assume_init() };
        if ["InitProc", "AIProc", "GetSaveRegions"].contains(&name.as_str()) {
            assert!(e.is_code, "{name} is a function");
        }
        if ["DescBlock", "LevelDesc", "MapName", "TechtreeName"].contains(&name.as_str()) {
            assert!(!e.is_code && e.rva != 0, "{name} is data");
        }
        names.push(name);
    }
    for name in ["DescBlock", "LevelDesc", "MapName", "TechtreeName", "InitProc", "AIProc"] {
        assert!(names.iter().any(|n| n == name), "{name}");
        assert!(out(|o| unsafe { op2f_mission_has_export(m, c(name).as_ptr(), o) }));
    }
    assert!(!out(|o| unsafe { op2f_mission_has_export(m, c("descblock").as_ptr(), o) }), "names are case-sensitive");
    let mut e = MaybeUninit::<Op2fMissionExport>::uninit();
    assert_eq!(unsafe { op2f_mission_export(m, desc.export_count, e.as_mut_ptr(), null_mut(), 0, null_mut()) }, Op2fStatus::OutOfRange);
    unsafe { op2f_mission_free(m) };
}

#[test]
fn not_a_mission_and_names() {
    let mut m = null_mut();
    assert_eq!(unsafe { op2f_mission_parse(b"MZ not really".as_ptr(), 13, &mut m) }, Op2fStatus::InvalidData);
    assert_eq!(unsafe { op2f_mission_parse(null(), 4, &mut m) }, Op2fStatus::NullArgument);

    let (mut kind, mut players) = (0, 0);
    ok(unsafe { op2f_mission_name_type(c("ml4_21.dll").as_ptr(), &mut kind, &mut players) });
    assert_eq!((kind, players), (OP2F_MISSION_LAST_ONE_STANDING, 4));
    assert_eq!(unsafe { op2f_mission_name_type(c("c_euf.dll").as_ptr(), &mut kind, &mut players) }, Op2fStatus::NotFound);

    let one = out(|o| unsafe { op2f_mission_checksum([1u8, 0, 0, 0].as_ptr(), 4, o) });
    assert_eq!(one, 0x8000_0000);
}
