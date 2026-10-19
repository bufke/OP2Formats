//! Extracting real Inno Setup installers. Neither kind can be committed, so both tests skip without them (even with
//! `OP2_REQUIRE_DATA`, which is about the game install):
//!
//! - GOG's Outpost 2 installer: `$OP2_GOG_INSTALLER`, else `~/Downloads/setup_outpost_2*.exe` (with any `-N.bin`
//!   parts beside it). Read only; extracted into a scratch folder that is removed afterwards.
//! - Dummy installers built with Inno Setup's compiler from a folder of arbitrary files: `$OP2_INNO_TEST_INSTALLERS`
//!   names a folder of `.exe` installers and `$OP2_INNO_TEST_SOURCE` the folder they were built from (the layout
//!   `tools/crates/op2-formats/tests/inno_test.iss` describes). Every installer must extract to exactly that folder.
#![cfg(feature = "inno")]

use std::fs;
use std::path::{Path, PathBuf};

use op2_formats::inno::Installer;
use op2_formats::install;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let p = std::env::temp_dir().join(format!("op2-inno-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn gog_installer() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OP2_GOG_INSTALLER").filter(|p| !p.is_empty()) {
        return Some(p.into());
    }
    let downloads = dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))?;
    let mut found: Vec<PathBuf> = fs::read_dir(downloads)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
            n.starts_with("setup_outpost_2") && n.ends_with(".exe")
        })
        .collect();
    found.sort();
    found.pop()
}

#[test]
fn gog_installer_extracts_to_a_complete_install() {
    let Some(path) = gog_installer() else {
        eprintln!("skipping: no GOG installer (set OP2_GOG_INSTALLER or put setup_outpost_2*.exe in ~/Downloads)");
        return;
    };
    let inst = Installer::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    eprintln!("{}: {} ({}), {} files, {} bytes", path.display(), inst.app_name, inst.version_text, inst.game_files().len(), inst.total_size());
    assert!(inst.app_name.starts_with("Outpost 2"), "{}", inst.app_name);
    let out = Scratch::new("gog");
    let mut last = 0;
    let done = inst.extract(&out.0, &mut |d, _| last = d).unwrap_or_else(|e| panic!("extracting: {e}"));
    assert_eq!(done.bytes, inst.total_size());
    assert_eq!(last, done.bytes);
    let check = install::check(&out.0);
    assert!(check.ok(), "missing {:?}", check.missing);
    assert_eq!(check.release, install::Release::Gog);
    eprintln!("extracted: {}", check.describe());
}

/// Every file below `dir`, relative, with its bytes.
fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((p.strip_prefix(base).unwrap().to_path_buf(), fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn dummy_installers_extract_exactly() {
    let (Some(dir), Some(src)) = (std::env::var_os("OP2_INNO_TEST_INSTALLERS"), std::env::var_os("OP2_INNO_TEST_SOURCE")) else {
        eprintln!("skipping: OP2_INNO_TEST_INSTALLERS / OP2_INNO_TEST_SOURCE not set");
        return;
    };
    let src = PathBuf::from(src);
    // The script installs the source folder, plus the English variant of a per-language file (lang/readme.txt).
    let mut want = tree(&src);
    want.push((PathBuf::from("lang/readme.txt"), fs::read(src.join("sub/deeper/Deep.TXT")).unwrap()));
    want.sort();
    let mut exes: Vec<PathBuf> = fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "exe")).collect();
    exes.sort();
    assert!(!exes.is_empty());
    for exe in exes {
        let inst = Installer::open(&exe).unwrap_or_else(|e| panic!("{}: {e}", exe.display()));
        let out = Scratch::new(&exe.file_stem().unwrap().to_string_lossy());
        inst.extract(&out.0, &mut |_, _| {}).unwrap_or_else(|e| panic!("{}: {e}", exe.display()));
        let got = tree(&out.0);
        let names = |t: &[(PathBuf, Vec<u8>)]| t.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
        assert_eq!(names(&got), names(&want), "{}", exe.display());
        for ((p, a), (_, b)) in got.iter().zip(&want) {
            assert!(a == b, "{}: {} differs", exe.display(), p.display());
        }
        eprintln!("{}: {} ok ({} files)", exe.display(), inst.version_text, got.len());
    }
}
