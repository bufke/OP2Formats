//! Installing OPU 1.4.1 from its real installer. It can't be committed, so the test skips without it (even with
//! `OP2_REQUIRE_DATA`, which is about the game install): `$OP2_OPU_PACKAGE`, else `~/Downloads/OPUInstaller-1.4.1.exe`
//! (download it from https://outpost2.net/outpost2.html). The game is the usual test install (`$OP2_DIR`, the source
//! tree's prefix). When that install has its own OPU made by OPU's installer, op2-rs's result must match it file for
//! file.
#![cfg(feature = "opu")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use op2_formats::opu;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OP2_OPU_PACKAGE").filter(|p| !p.is_empty()) {
        return Some(p.into());
    }
    let downloads = dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))?;
    Some(downloads.join(opu::PACKAGE_NAME)).filter(|p| p.is_file())
}

/// Relative path (with `/`) -> SHA-1 of every file below `dir`.
fn tree(dir: &Path, rel: &str, out: &mut BTreeMap<String, String>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if e.path().is_dir() {
            tree(&e.path(), &r, out);
        } else {
            out.insert(r, sha1_smol::Sha1::from(fs::read(e.path()).unwrap()).digest().to_string());
        }
    }
}

#[test]
fn opu_installer_installs_what_opus_own_installer_does() {
    let Some(pkg) = package() else {
        eprintln!("skipping: no OPU installer (set OP2_OPU_PACKAGE or put {} in ~/Downloads)", opu::PACKAGE_NAME);
        return;
    };
    let Some(game) = op2_formats::testdata::game_dir() else { return };
    let out = Scratch(std::env::temp_dir().join(format!("op2-opu-{}", std::process::id())));
    let target = out.0.join("OPU");
    let mut stages = Vec::new();
    let installed = opu::install(&pkg, &game, &target, &mut |s, _, _| {
        if stages.last() != Some(&s) {
            stages.push(s);
        }
    })
    .unwrap();
    assert_eq!(stages, [opu::Stage::Read, opu::Stage::Install]);
    assert!(installed.warnings.is_empty(), "{:?}", installed.warnings);
    assert!(!out.0.join("OPU.partial").exists());
    let mut ours = BTreeMap::new();
    tree(&target, "", &mut ours);
    assert_eq!(ours.len(), installed.files);
    eprintln!("installed {} files, tree {}", ours.len(), opu::tree_digest(&target).unwrap());
    for f in ["README.md", "LICENSE.txt", "CHANGES.md", "OPUPatch.dll", "base/sheets/vehicles.txt", "base/techs/multitek.txt", "maps/Swarm/Swarm.map"] {
        assert!(ours.contains_key(f), "{f} missing");
    }
    assert_eq!(op2_formats::install::opu_version_of(&target).as_deref(), Some(opu::VERSION));

    // The game's own OPU, made by OPU's installer: every file of ours is there, byte for byte, and every OPU file
    // there is ours (what the game, the player and the harness made there aside).
    let Some(theirs_dir) = opu::find_opu_folder(&game) else {
        eprintln!("the test install has no OPU of its own to compare with");
        return;
    };
    let mut theirs = BTreeMap::new();
    tree(&theirs_dir, "", &mut theirs);
    for (path, sha) in &ours {
        assert_eq!(theirs.get(path), Some(sha), "{path} differs from {}", theirs_dir.display());
    }
    let not_opu = |p: &str| {
        let top = p.split('/').next().unwrap();
        ["saves", "backups", "Harness", "Outpost2Log.txt", "outpost2.ini", "outpost2.ini.pre-harness"].contains(&top)
            // This project's own missions (missions/Makefile installs them there).
            || p.starts_with("maps/ClaudeTest/")
            || p.starts_with("maps/RivalColony/")
    };
    let missing: Vec<&String> = theirs.keys().filter(|p| !ours.contains_key(*p) && !not_opu(p)).collect();
    assert!(missing.is_empty(), "not installed: {missing:?}");
}

#[test]
fn a_file_that_isnt_the_pinned_installer_is_refused() {
    let dir = Scratch(std::env::temp_dir().join(format!("op2-opu-bad-{}", std::process::id())));
    fs::create_dir_all(&dir.0).unwrap();
    let fake = dir.0.join(opu::PACKAGE_NAME);
    fs::write(&fake, b"MZ not OPU").unwrap();
    let err = opu::install(&fake, &dir.0, &dir.0.join("OPU"), &mut |_, _, _| {}).unwrap_err().to_string();
    assert!(err.contains("isn't OPU 1.4.1's installer"), "{err}");
    assert!(!dir.0.join("OPU").exists() && !dir.0.join("OPU.partial").exists());
}
