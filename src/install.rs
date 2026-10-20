//! Finding the player's own Outpost 2 install: the places GOG, the original CD and the usual Wine/Proton front ends
//! put it, checking a folder holds what op2-rs needs, and telling the GOG release, a CD install and the OPU update
//! apart. docs/game-data.md lists the places for users.
//!
//! Nothing here downloads anything: op2-rs only reads the copy the player already has (docs/legal.md rule 1).

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// GOG's product id for Outpost 2, used in its registry key, `goggame-<id>.info` and Heroic's library.
pub const GOG_GAME_ID: &str = "1494370738";
/// The only place op2-rs points players to for buying the game (docs/legal.md rule 1).
pub const GOG_STORE_URL: &str = "https://www.gog.com/en/game/outpost_2_divided_destiny";

/// GOG's `Outpost2.exe` (version 1.2.0.7, as listed in the release's `goggame-1494370738.hashdb`), the build the
/// addresses in this project were read from.
const GOG_EXE_SHA1: &str = "1c7d63a05beae42af441e0955d0a9420fae2f85e";

/// Files op2-rs can't run without, with what each is for.
pub const REQUIRED: &[(&str, &str)] = &[
    ("Outpost2.exe", "the game's tables are read from it"),
    ("op2shres.dll", "menu and in-game interface art"),
    ("OP2Shell.dll", "the main menu"),
    ("out2res.dll", "interface resources"),
    ("maps.vol", "maps, tilesets and unit art"),
    ("sheets.vol", "unit and building rules"),
    ("OP2_ART.BMP", "unit and building sprites"),
    ("e01.dll", "the campaign missions"),
];

/// Files op2-rs uses when present; it runs without them.
pub const OPTIONAL: &[(&str, &str)] = &[
    ("op2.clm", "music"),
    ("sound.vol", "sound effects"),
    ("voices.vol", "voice messages"),
    ("story.vol", "mission briefings"),
    ("I.avi", "the intro movie"),
];

/// Which release a folder looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// GOG's release: it has `goggame-1494370738.info`.
    Gog,
    /// No GOG files: the original CD release, or a copy of one.
    Cd,
}

/// What a folder holds and what it's missing.
#[derive(Clone, Debug)]
pub struct Check {
    pub dir: PathBuf,
    /// Required files (names from [`REQUIRED`]) not found.
    pub missing: Vec<&'static str>,
    /// Optional files not found.
    pub missing_optional: Vec<&'static str>,
    pub release: Release,
    /// The OPU community update: `Some(version)` when installed (`"1.4.1"`, or `""` when its version isn't readable).
    pub opu: Option<String>,
    /// The OPU folder layered over the game ([`opu_dir`]): the game folder's own `OPU`, or op2-rs's install of it.
    pub opu_dir: Option<PathBuf>,
    /// `Outpost2.exe`'s FileVersion, when readable.
    pub exe_version: Option<String>,
    /// Whether `Outpost2.exe` is byte-identical to GOG's (`None` when it's missing).
    pub exe_matches_gog: Option<bool>,
}

impl Check {
    /// Whether op2-rs can run from this folder.
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
    }

    /// The release in a few words: "GOG 1.2 + OPU 1.4.1" (the target), "GOG 1.2, no OPU", "CD release + OPU 1.4.1".
    pub fn summary(&self) -> String {
        let mut s = match (self.release, self.exe_matches_gog) {
            (Release::Gog, Some(true)) => "GOG 1.2".to_string(),
            (Release::Gog, _) => "GOG release".to_string(),
            (Release::Cd, _) => "CD release".to_string(),
        };
        match self.opu.as_deref() {
            Some("") => s.push_str(" + OPU"),
            Some(v) => s.push_str(&format!(" + OPU {v}")),
            None => s.push_str(", no OPU"),
        }
        s
    }

    /// One line describing the release, e.g. "GOG 1.2 + OPU 1.4.1 (Outpost2.exe 1.2.0.7, GOG's)".
    pub fn describe(&self) -> String {
        let mut details = Vec::new();
        if let Some(gog) = self.exe_matches_gog {
            let v = self.exe_version.as_deref().map(|v| format!(" {v}")).unwrap_or_default();
            details.push(format!("Outpost2.exe{v}, {}", if gog { "GOG's" } else { "not GOG's build" }));
        }
        if self.release == Release::Cd {
            details.push("no GOG files".into());
        }
        if let Some(d) = &self.opu_dir
            && !d.starts_with(&self.dir)
        {
            details.push(format!("OPU installed by op2-rs in {}", d.display()));
        }
        let mut s = self.summary();
        if !details.is_empty() {
            s.push_str(&format!(" ({})", details.join("; ")));
        }
        s
    }

    /// A one-line form of [`problems`](Self::problems) for tight spaces: "missing maps.vol, e01.dll".
    pub fn short_problem(&self) -> String {
        if self.missing.is_empty() {
            return self.problems().join("; ");
        }
        if self.missing.len() == REQUIRED.len() {
            return "no Outpost 2 files in it".into();
        }
        format!("missing {}", self.missing.join(", "))
    }

    /// Problems worth telling the player about, one per line (empty when there are none).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for name in &self.missing {
            let why = REQUIRED.iter().find(|(n, _)| n == name).map(|(_, w)| *w).unwrap_or("");
            out.push(format!("missing {name} ({why})"));
        }
        if self.exe_matches_gog == Some(false) {
            out.push(
                "Outpost2.exe isn't GOG's build: op2-rs reads some tables from it and is tested with GOG's 1.2.0.7; \
                 another version may show wrong sounds or animations"
                    .to_string(),
            );
        }
        out
    }
}

/// Checks `dir` for the files op2-rs needs. File names match case-insensitively, as on Windows.
pub fn check(dir: &Path) -> Check {
    check_with_opu(dir, opu_dir(dir))
}

/// [`check`] with the OPU folder given (tests: independent of op2-rs's own installed OPU on this machine).
fn check_with_opu(dir: &Path, opu_dir: Option<PathBuf>) -> Check {
    let names = dir_index(dir);
    let has = |name: &str| names.contains_key(&name.to_ascii_lowercase());
    let missing = REQUIRED.iter().map(|(n, _)| *n).filter(|n| !has(n)).collect();
    let missing_optional = OPTIONAL.iter().map(|(n, _)| *n).filter(|n| !has(n)).collect();
    let release = if has(&format!("goggame-{GOG_GAME_ID}.info")) { Release::Gog } else { Release::Cd };
    let opu = opu_dir.as_deref().and_then(opu_version_of);
    let exe = names.get("outpost2.exe").and_then(|p| fs::read(p).ok());
    let exe_version = exe.as_deref().and_then(exe_file_version);
    let exe_matches_gog = exe.as_deref().map(|d| sha1_smol::Sha1::from(d).digest().to_string() == GOG_EXE_SHA1);
    Check { dir: dir.to_path_buf(), missing, missing_optional, release, opu, opu_dir, exe_version, exe_matches_gog }
}

/// Where op2-rs installs OPU when the game folder has none: `<data folder>/op2-rs/OPU`, beside its copy of the game
/// (docs/game-data.md). It holds what a game folder's `OPU` subfolder would.
pub fn opu_install_dir(env: &Env) -> Option<PathBuf> {
    env.op2_data().map(|d| d.join("OPU"))
}

/// The OPU folder layered over the game in `game` (by [`GameFiles`](crate::GameFiles), mission lookup and the
/// checks): its own `OPU` subfolder when it has one, as the original game with OPU does; else, for a game folder
/// (one with `Outpost2.exe`), the OPU op2-rs installed ([`opu_install_dir`]), if complete.
pub fn opu_dir(game: &Path) -> Option<PathBuf> {
    opu_dir_with(game, opu_install_dir(&Env::current()).as_deref())
}

/// [`opu_dir`] with op2-rs's own OPU folder given.
pub fn opu_dir_with(game: &Path, installed: Option<&Path>) -> Option<PathBuf> {
    if let Some(own) = resolve_ci(game, Path::new("OPU")).filter(|p| p.is_dir()) {
        return Some(own);
    }
    let installed = installed?;
    let is_game = crate::find_in_dir(game, "Outpost2.exe").is_ok();
    (is_game && crate::find_in_dir(installed, "OPUPatch.dll").is_ok()).then(|| installed.to_path_buf())
}

/// Lowercase file name -> path, for the files directly in `dir`.
fn dir_index(dir: &Path) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        out.entry(e.file_name().to_string_lossy().to_ascii_lowercase()).or_insert_with(|| e.path());
    }
    out
}

/// The version of the OPU update in folder `opu`, from its `CHANGES.md` ("1.4.1") or `README.md` ("1.4.x"); `""` when
/// it has OPU's files but no readable version, `None` when it isn't OPU (no `OPUPatch.dll`, nor the game folder's
/// `op2ext.dll` beside it).
pub fn opu_version_of(opu: &Path) -> Option<String> {
    let files = dir_index(opu);
    let loader = opu.parent().is_some_and(|game| dir_index(game).contains_key("op2ext.dll"));
    if !files.contains_key("opupatch.dll") && !loader {
        return None;
    }
    let version = ["changes.md", "readme.md"]
        .iter()
        .filter_map(|n| files.get(*n))
        .filter_map(|p| fs::read_to_string(p).ok())
        .find_map(|text| first_version(&text))
        .unwrap_or_default();
    Some(version)
}

/// The first "N.N.x" or "N.N.N" in `text`.
fn first_version(text: &str) -> Option<String> {
    let b = text.as_bytes();
    for start in 0..b.len() {
        if !b[start].is_ascii_digit() || (start > 0 && (b[start - 1].is_ascii_digit() || b[start - 1] == b'.')) {
            continue;
        }
        let end = start + b[start..].iter().take_while(|c| c.is_ascii_digit() || **c == b'.' || **c == b'x').count();
        let v = text[start..end].trim_end_matches('.');
        if v.split('.').count() >= 3 && v.split('.').all(|p| !p.is_empty()) {
            return Some(v.to_string());
        }
    }
    None
}

/// `FileVersion` from a PE file's version resource.
fn exe_file_version(data: &[u8]) -> Option<String> {
    let res = crate::pe_res::PeResources::parse(data.to_vec()).ok()?;
    let kind = crate::pe_res::ResId::Id(crate::pe_res::RT_VERSION);
    let r = res.of_type(&kind).next()?;
    crate::pe_res::version_strings(res.bytes(r)).into_iter().find(|(k, _)| k == "FileVersion").map(|(_, v)| v)
}

/// A folder to look in, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub dir: PathBuf,
    /// Where the candidate came from, for messages ("GOG default folder", "Wine prefix ~/.wine", ...).
    pub origin: String,
}

/// What discovery may look at; tests build one over a fake home folder.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub os: Os,
    pub home: Option<PathBuf>,
    /// The platform's per-user data folder (`dirs::data_local_dir`), where op2-rs extracts an installer.
    pub data_local: Option<PathBuf>,
    /// The platform's per-user config folder (`dirs::config_dir`).
    pub config: Option<PathBuf>,
    /// Environment variables discovery reads (`WINEPREFIX`, `ProgramFiles`, ...).
    pub vars: HashMap<String, OsString>,
    /// Install folders read from the Windows registry (Windows only).
    pub registry: Vec<Candidate>,
    /// op2-rs's own data folder when set with [`set_data_dir`] (`--data-dir`), instead of `<data_local>/op2-rs`.
    pub data_override: Option<PathBuf>,
}

static DATA_DIR: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// The command-line flag the programs accept for [`set_data_dir`].
pub const DATA_DIR_FLAG: &str = "--data-dir";

/// Uses `dir` as op2-rs's data folder (where it unpacks GOG's installer and installs OPU) for the rest of the
/// process, instead of the platform's.
pub fn set_data_dir(dir: PathBuf) {
    *DATA_DIR.write().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Os {
    #[default]
    Linux,
    MacOs,
    Windows,
}

impl Env {
    /// The real environment of this process.
    pub fn current() -> Env {
        let os = if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        };
        let vars = ["WINEPREFIX", "ProgramFiles", "ProgramFiles(x86)", "SystemDrive", "XDG_DATA_HOME", "XDG_CONFIG_HOME"]
            .iter()
            .filter_map(|k| std::env::var_os(k).map(|v| (k.to_string(), v)))
            .collect();
        Env {
            os,
            home: dirs::home_dir(),
            data_local: dirs::data_local_dir(),
            config: dirs::config_dir(),
            vars,
            registry: registry_candidates(),
            data_override: DATA_DIR.read().unwrap_or_else(|e| e.into_inner()).clone(),
        }
    }

    fn var(&self, k: &str) -> Option<PathBuf> {
        self.vars.get(k).filter(|v| !v.is_empty()).map(PathBuf::from)
    }

    /// op2-rs's own data folder: `<data folder>/op2-rs`, or the one given with [`set_data_dir`].
    pub fn op2_data(&self) -> Option<PathBuf> {
        self.data_override.clone().or_else(|| self.data_local.as_ref().map(|d| d.join("op2-rs")))
    }
}

/// Where op2-rs extracts the GOG installer: `<data folder>/op2-rs/Outpost 2`.
pub fn extract_dir(env: &Env) -> Option<PathBuf> {
    env.op2_data().map(|d| d.join("Outpost 2"))
}

/// Folders inside a Windows drive (or a Wine prefix's `drive_c`) where Outpost 2 is commonly installed.
const DRIVE_FOLDERS: &[(&[&str], &str)] = &[
    (&["GOG Games", "Outpost 2"], "GOG default folder"),
    (&["Program Files (x86)", "GOG Galaxy", "Games", "Outpost 2"], "GOG Galaxy default folder"),
    (&["Program Files", "GOG Galaxy", "Games", "Outpost 2"], "GOG Galaxy default folder"),
    (&["Sierra", "Outpost 2"], "CD default folder"),
    (&["Program Files (x86)", "Sierra", "Outpost 2"], "CD install under Program Files"),
    (&["Program Files", "Sierra", "Outpost 2"], "CD install under Program Files"),
    (&["Games", "Outpost 2"], "Games folder"),
];

/// Every place discovery looks, in order, before checking which exist.
pub fn known_locations(env: &Env) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut push = |dir: PathBuf, origin: String| out.push(Candidate { dir, origin });
    if let Some(d) = extract_dir(env) {
        push(d, "extracted by op2-rs from the GOG installer".into());
    }
    match env.os {
        Os::Windows => {
            for c in &env.registry {
                push(c.dir.clone(), c.origin.clone());
            }
            let system = env.var("SystemDrive").unwrap_or_else(|| PathBuf::from("C:"));
            let mut drives = vec![system];
            drives.extend("CDEFGH".chars().map(|l| PathBuf::from(format!("{l}:"))));
            drives.dedup_by(|a, b| a.as_os_str().eq_ignore_ascii_case(b.as_os_str()));
            for pf in ["ProgramFiles(x86)", "ProgramFiles"] {
                if let Some(pf) = env.var(pf) {
                    push(pf.join("GOG Galaxy").join("Games").join("Outpost 2"), "GOG Galaxy default folder".into());
                    push(pf.join("Sierra").join("Outpost 2"), "CD install under Program Files".into());
                }
            }
            for drive in drives {
                let root = PathBuf::from(format!("{}\\", drive.display()));
                for (rel, origin) in DRIVE_FOLDERS {
                    push(rel.iter().fold(root.clone(), |p, c| p.join(c)), (*origin).into());
                }
            }
        }
        Os::Linux | Os::MacOs => {
            for (prefix, why) in wine_prefixes(env) {
                for c in prefix_candidates(&prefix, &why) {
                    push(c.dir, c.origin);
                }
            }
        }
    }
    if let Some(home) = &env.home {
        // Native-looking folders: minigalaxy unpacks Windows games into ~/GOG Games, Heroic into ~/Games/Heroic.
        push(home.join("GOG Games").join("Outpost 2"), "~/GOG Games (minigalaxy, innoextract)".into());
        push(home.join("Games").join("Heroic").join("Outpost 2"), "Heroic's default library".into());
        push(home.join("Games").join("Outpost 2"), "~/Games".into());
    }
    for dir in heroic_install_paths(env) {
        push(dir, "Heroic's GOG library".into());
    }
    for (dir, why) in lutris_paths(env) {
        if why == "prefix" {
            for c in prefix_candidates(&dir, &format!("Lutris prefix {}", dir.display())) {
                push(c.dir, c.origin);
            }
        } else {
            push(dir, "Lutris game".into());
        }
    }
    // Same folder reached two ways: keep the first.
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.dir.clone()));
    out
}

/// Wine prefixes to look inside (folders with a `drive_c`), with a description.
fn wine_prefixes(env: &Env) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if let Some(p) = env.var("WINEPREFIX") {
        out.push((p, "$WINEPREFIX".to_string()));
    }
    let Some(home) = &env.home else { return out };
    out.push((home.join(".wine"), "Wine prefix ~/.wine".into()));
    // (folder holding prefixes, how deep the prefixes sit, description)
    let mut parents: Vec<(PathBuf, usize, &str)> = vec![
        (home.join("Games"), 1, "Lutris/Heroic games folder"),
        (home.join("Games").join("Heroic").join("Prefixes"), 2, "Heroic prefix"),
        (home.join(".local/share/bottles/bottles"), 1, "Bottles"),
        (home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"), 1, "Bottles (Flatpak)"),
        (home.join(".PlayOnLinux/wineprefix"), 1, "PlayOnLinux"),
        (home.join(".cxoffice"), 1, "CrossOver"),
        (home.join(".steam/steam/steamapps/compatdata"), 2, "Proton prefix"),
        (home.join(".local/share/Steam/steamapps/compatdata"), 2, "Proton prefix"),
    ];
    if env.os == Os::MacOs {
        parents.push((home.join("Library/Application Support/CrossOver/Bottles"), 1, "CrossOver bottle"));
        parents.push((home.join("Library/Containers/com.isaacmarovitz.Whisky/Bottles"), 1, "Whisky bottle"));
        // Wineskin and Porting Kit wrappers keep their prefix inside the app bundle.
        for apps in [home.join("Applications"), home.join("Applications/Wineskin"), PathBuf::from("/Applications")] {
            for app in read_dirs(&apps) {
                out.push((app.join("Contents/SharedSupport/prefix"), format!("wrapper {}", app.display())));
            }
        }
    }
    for (parent, depth, why) in parents {
        let mut level = vec![parent];
        for _ in 0..depth {
            level = level.iter().flat_map(|d| read_dirs(d)).collect();
            for d in &level {
                if is_prefix(d) {
                    out.push((d.clone(), format!("{why} {}", d.display())));
                } else if is_prefix(&d.join("pfx")) {
                    out.push((d.join("pfx"), format!("{why} {}", d.display())));
                }
            }
        }
    }
    // ~/.steam/steam is usually a link to ~/.local/share/Steam: keep each prefix once.
    let mut seen = std::collections::HashSet::new();
    out.retain(|(p, _)| is_prefix(p) && seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())));
    out
}

fn is_prefix(dir: &Path) -> bool {
    dir.join("drive_c").is_dir()
}

/// Subfolders of `dir`, sorted (none if it isn't readable).
fn read_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// Install folders inside one Wine prefix: the registry's (GOG's key and uninstall entries in `system.reg`), then
/// the default folders.
fn prefix_candidates(prefix: &Path, why: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Ok(reg) = fs::read(prefix.join("system.reg")) {
        for win in registry_paths_in_reg_file(&String::from_utf8_lossy(&reg)) {
            if let Some(dir) = wine_path(prefix, &win) {
                out.push(Candidate { dir, origin: format!("registry of {why}") });
            }
        }
    }
    let drive_c = prefix.join("drive_c");
    for (rel, origin) in DRIVE_FOLDERS {
        out.push(Candidate { dir: rel.iter().fold(drive_c.clone(), |p, c| p.join(c)), origin: format!("{origin} in {why}") });
    }
    out
}

/// Maps a Windows path inside a Wine prefix (`C:\GOG Games\Outpost 2`) to the host path.
pub fn wine_path(prefix: &Path, win: &str) -> Option<PathBuf> {
    let win = win.trim().trim_end_matches('\\');
    let (drive, rest) = win.split_once(':')?;
    if drive.len() != 1 {
        return None;
    }
    let root = if drive.eq_ignore_ascii_case("c") {
        prefix.join("drive_c")
    } else {
        // One join: a lone `d:` would read as a drive on a Windows host and replace the whole path.
        prefix.join(format!("dosdevices/{}:", drive.to_ascii_lowercase()))
    };
    Some(rest.split('\\').filter(|c| !c.is_empty()).fold(root, |p, c| p.join(c)))
}

/// Outpost 2 install folders recorded in a Wine `system.reg`: GOG's `GOG.com\Games\1494370738` key (`path`) and any
/// uninstall entry whose DisplayName is Outpost 2 (`InstallLocation`, Inno's `Inno Setup: App Path`).
pub fn registry_paths_in_reg_file(reg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let gog = format!("gog.com\\\\games\\\\{GOG_GAME_ID}]");
    let mut section = String::new();
    let mut values: Vec<(String, String)> = Vec::new();
    let mut flush = |section: &str, values: &mut Vec<(String, String)>| {
        let s = section.to_ascii_lowercase();
        let get = |k: &str| values.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone());
        if s.contains(&gog) {
            out.extend(get("path"));
        } else if s.contains("\\\\uninstall\\\\")
            && get("DisplayName").is_some_and(|n| n.to_ascii_lowercase().starts_with("outpost 2"))
        {
            out.extend(get("InstallLocation").or_else(|| get("Inno Setup: App Path")));
        }
        values.clear();
    };
    for line in reg.lines() {
        if line.starts_with('[') {
            flush(&section, &mut values);
            section = line.to_string();
        } else if let Some((k, v)) = line.split_once("\"=\"") {
            let k = k.trim_start_matches('"');
            let v = v.strip_suffix('"').unwrap_or(v);
            values.push((unescape_reg(k), unescape_reg(v)));
        }
    }
    flush(&section, &mut values);
    out.retain(|p| !p.is_empty());
    out.dedup();
    out
}

fn unescape_reg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `install_path` values from Heroic's GOG library (`gog_store/installed.json`), native or Flatpak.
fn heroic_install_paths(env: &Env) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(c) = &env.config {
        files.push(c.join("heroic/gog_store/installed.json"));
    }
    if let Some(h) = &env.home {
        files.push(h.join(".config/heroic/gog_store/installed.json"));
        files.push(h.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/gog_store/installed.json"));
    }
    files.dedup();
    files.iter().filter_map(|f| fs::read_to_string(f).ok()).flat_map(|t| json_string_values(&t, "install_path")).map(PathBuf::from).collect()
}

/// Every string value of `key` in a JSON text (a light scan: Heroic's file is a flat list of objects).
pub fn json_string_values(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else { continue };
        let after = after.trim_start();
        let Some(body) = after.strip_prefix('"') else { continue };
        let mut value = String::new();
        let mut chars = body.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('u') => {
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            value.push(ch);
                        }
                    }
                    Some(other) => value.push(other),
                    None => break,
                },
                c => value.push(c),
            }
        }
        out.push(value);
    }
    out
}

/// Game folders and prefixes from Lutris's game configs (`exe:` and `prefix:` lines in `games/*.yml`) for games
/// whose config mentions Outpost 2.
fn lutris_paths(env: &Env) -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(c) = &env.config {
        dirs.push(c.join("lutris/games"));
    }
    if let Some(h) = &env.home {
        dirs.push(h.join(".config/lutris/games"));
        dirs.push(h.join(".local/share/lutris/games"));
    }
    dirs.dedup();
    let mut out = Vec::new();
    for d in dirs {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(text) = fs::read_to_string(e.path()) else { continue };
            if !text.to_ascii_lowercase().contains("outpost") {
                continue;
            }
            for line in text.lines() {
                let line = line.trim();
                let value = |k: &str| line.strip_prefix(k).map(|v| v.trim().trim_matches(['"', '\'']).to_string());
                if let Some(exe) = value("exe:") {
                    if let Some(parent) = Path::new(&exe).parent() {
                        out.push((parent.to_path_buf(), "game"));
                    }
                } else if let Some(prefix) = value("prefix:") {
                    out.push((PathBuf::from(prefix), "prefix"));
                }
            }
        }
    }
    out
}

/// Resolves `rel` under `base` matching each component case-insensitively (a Wine prefix on a case-sensitive
/// filesystem may hold `GOG Games` as `gog games`). The exact spelling wins when it exists.
pub fn resolve_ci(base: &Path, rel: &Path) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for comp in rel.components() {
        let name = comp.as_os_str();
        let exact = cur.join(name);
        if exact.exists() {
            cur = exact;
            continue;
        }
        let want = name.to_string_lossy().to_lowercase();
        cur = fs::read_dir(&cur).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == want)?.path();
    }
    Some(cur)
}

/// Finds the folder on disk for a candidate, allowing case differences below the first existing ancestor.
fn locate(dir: &Path) -> Option<PathBuf> {
    if dir.is_dir() {
        return Some(dir.to_path_buf());
    }
    let mut base = dir;
    let mut rel = Vec::new();
    while !base.is_dir() {
        rel.push(base.file_name()?);
        base = base.parent()?;
    }
    let rel: PathBuf = rel.iter().rev().collect();
    resolve_ci(base, &rel).filter(|p| p.is_dir())
}

/// The result of looking through the known locations.
#[derive(Debug, Default)]
pub struct Search {
    /// Every place looked at.
    pub searched: Vec<Candidate>,
    /// Folders that exist and have `Outpost2.exe`, with their check, in search order.
    pub found: Vec<(Candidate, Check)>,
}

impl Search {
    /// The first complete install.
    pub fn best(&self) -> Option<&(Candidate, Check)> {
        self.found.iter().find(|(_, c)| c.ok())
    }
}

/// Looks through [`known_locations`].
pub fn search(env: &Env) -> Search {
    let searched = known_locations(env);
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in &searched {
        let Some(dir) = locate(&c.dir) else { continue };
        if !seen.insert(dir.clone()) {
            continue;
        }
        if dir_index(&dir).contains_key("outpost2.exe") {
            found.push((Candidate { dir: dir.clone(), origin: c.origin.clone() }, check(&dir)));
        }
    }
    Search { searched, found }
}

/// The install a player means by choosing `dir`: the folder itself, else (when they picked a level too high) a Wine
/// prefix's install, an `Outpost 2` subfolder or any subfolder with `Outpost2.exe`. Returns the check of the best
/// folder found, complete or not (`dir` itself when nothing better turns up).
pub fn check_chosen(dir: &Path) -> Check {
    let own = check(dir);
    if own.ok() {
        return own;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if is_prefix(dir) {
        candidates.extend(prefix_candidates(dir, "").into_iter().map(|c| c.dir));
    }
    candidates.push(dir.join("Outpost 2"));
    candidates.extend(read_dirs(dir));
    let mut partial = None;
    for c in candidates {
        let Some(c) = locate(&c) else { continue };
        if !dir_index(&c).contains_key("outpost2.exe") {
            continue;
        }
        let chk = check(&c);
        if chk.ok() {
            return chk;
        }
        partial.get_or_insert(chk);
    }
    if own.exe_matches_gog.is_some() { own } else { partial.unwrap_or(own) }
}

/// Where the game folder came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    CommandLine,
    EnvVar,
    /// This repository's Wine prefix, for builds run from the source tree.
    SourceTree,
    Settings,
    Found(String),
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::CommandLine => write!(f, "--game-dir"),
            Origin::EnvVar => write!(f, "$OP2_DIR"),
            Origin::SourceTree => write!(f, "the source tree's Wine prefix"),
            Origin::Settings => write!(f, "game_dir in settings.toml"),
            Origin::Found(why) => write!(f, "found: {why}"),
        }
    }
}

/// The game folder to use and why, or why there isn't one.
#[derive(Debug)]
pub enum Resolved {
    /// A folder to use. An explicit choice (command line, `$OP2_DIR`, settings) is returned even when its check
    /// fails, so the caller can say what's wrong with it.
    Dir { check: Check, origin: Origin },
    /// Nothing given and nothing found.
    NotFound(Search),
}

/// Picks the game folder: `--game-dir`, then `$OP2_DIR`, then (for a build run from this source tree) the repo's
/// Wine prefix, then the saved `game_dir` setting, then the first complete install in [`known_locations`].
/// A saved folder that has since disappeared is skipped.
pub fn resolve(cli: Option<PathBuf>, saved: Option<PathBuf>) -> Resolved {
    if let Some(d) = cli {
        return Resolved::Dir { check: check(&d), origin: Origin::CommandLine };
    }
    if let Some(d) = std::env::var_os("OP2_DIR").filter(|v| !v.is_empty()) {
        return Resolved::Dir { check: check(Path::new(&d)), origin: Origin::EnvVar };
    }
    if let Some(d) = crate::source_tree_game_dir() {
        return Resolved::Dir { check: check(&d), origin: Origin::SourceTree };
    }
    if let Some(d) = saved.filter(|d| d.is_dir()) {
        return Resolved::Dir { check: check(&d), origin: Origin::Settings };
    }
    let s = search(&Env::current());
    match s.best() {
        Some((c, chk)) => Resolved::Dir { check: chk.clone(), origin: Origin::Found(c.origin.clone()) },
        None => Resolved::NotFound(s),
    }
}

/// What to do about OPU ([`crate::opu`]) once there's a usable game folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpuPlan {
    /// OPU is there (in the game folder, or installed by op2-rs): use it without asking.
    Ready,
    /// Download and install it: asked for (`--fetch-opu`, `fetch_opu` in settings.toml) or
    /// [`FETCH_BY_DEFAULT`](crate::opu::FETCH_BY_DEFAULT).
    Fetch,
    /// Ask the player to get it: the first-run screen, `op2 setup`.
    Ask,
    /// Run without it, with a warning: the developer and test paths (`--game-dir`, `$OP2_DIR`, the source tree), so
    /// tests with a plain game folder keep working.
    Warn,
}

/// The [`OpuPlan`] for `check`, a folder from `origin`; `fetch` is the player's request to download OPU.
pub fn opu_plan(check: &Check, origin: &Origin, fetch: bool) -> OpuPlan {
    if check.opu.is_some() {
        OpuPlan::Ready
    } else if fetch {
        OpuPlan::Fetch
    } else if matches!(origin, Origin::CommandLine | Origin::EnvVar | Origin::SourceTree) {
        OpuPlan::Warn
    } else if crate::opu::FETCH_BY_DEFAULT {
        OpuPlan::Fetch
    } else {
        OpuPlan::Ask
    }
}

/// The text for a game folder without OPU: what OPU is, who made it, where to get it and how to give it to op2-rs.
pub fn opu_needed_message(check: &Check) -> String {
    use crate::opu;
    format!(
        "{} has no OPU update. op2-rs needs it: it plays Outpost 2 as updated by OPU {}, the free community update.\n\
         \n{}.\n\
         Download it from {}, then:  op2 setup --opu \"<{}>\"\n\
         (or an OPU folder). op2-rs installs it in its own data folder; your game folder isn't changed.\n",
        check.dir.display(),
        opu::VERSION,
        opu::CREDIT,
        opu::DOWNLOAD_PAGE,
        opu::PACKAGE_NAME,
    )
}

/// For the command-line programs: [`resolve`]s the folder and makes it [`default_game_dir`](crate::default_game_dir)
/// for the process. An incomplete explicit choice gets a warning on stderr (a tool may need only part of the game);
/// with nothing found, the error is [`not_found_message`].
pub fn use_for_tool(cli: Option<PathBuf>, saved: Option<PathBuf>) -> Result<PathBuf, String> {
    match resolve(cli, saved) {
        Resolved::Dir { check, origin } => {
            for p in check.problems() {
                eprintln!("warning: game folder {} ({origin}): {p}", check.dir.display());
            }
            if check.ok() && check.opu.is_none() {
                eprintln!(
                    "warning: game folder {} ({origin}) has no OPU {} update, which op2-rs follows; see `op2 setup`",
                    check.dir.display(),
                    crate::opu::VERSION
                );
            }
            crate::set_game_dir(check.dir.clone());
            Ok(check.dir)
        }
        Resolved::NotFound(_) => Err(not_found_message(None)),
    }
}

/// Extracts GOG's installer into `target` and checks the result. Extraction goes to `<target>.partial` first and is
/// renamed into place only when complete. `target` must be empty or absent, or be op2-rs's own [`extract_dir`]
/// (which is replaced).
#[cfg(feature = "inno")]
pub fn extract_installer(installer: &Path, target: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<Check, crate::Error> {
    use crate::Error;
    let io = |p: &Path, e: std::io::Error| Error::Io { path: p.display().to_string(), source: e };
    let inst = crate::inno::Installer::open(installer)?;
    if !inst.app_name.to_ascii_lowercase().starts_with("outpost 2") {
        return Err(Error::Invalid(format!("{} installs {:?}, not Outpost 2", installer.display(), inst.app_name)));
    }
    let ours = extract_dir(&Env::current()).is_some_and(|d| d == target);
    let occupied = fs::read_dir(target).is_ok_and(|mut d| d.next().is_some());
    if occupied && !ours {
        return Err(Error::Invalid(format!("{} isn't empty; choose an empty or new folder", target.display())));
    }
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "game".into());
    let partial = target.with_file_name(format!("{name}.partial"));
    if partial.exists() {
        fs::remove_dir_all(&partial).map_err(|e| io(&partial, e))?;
    }
    fs::create_dir_all(&partial).map_err(|e| io(&partial, e))?;
    if let Err(e) = inst.extract(&partial, progress) {
        let _ = fs::remove_dir_all(&partial);
        return Err(e);
    }
    let chk = check(&partial);
    if !chk.ok() {
        let _ = fs::remove_dir_all(&partial);
        return Err(Error::Invalid(format!("the installer didn't contain {}", chk.missing.join(", "))));
    }
    if target.exists() {
        fs::remove_dir_all(target).map_err(|e| io(target, e))?;
    }
    fs::rename(&partial, target).map_err(|e| io(target, e))?;
    Ok(check(target))
}

/// The text shown when there's no usable install: what op2-rs needs, where to buy the game, how to point op2-rs at
/// a copy. `problem` says what was wrong with a folder that was given, if any.
pub fn not_found_message(problem: Option<&Check>) -> String {
    let mut s = String::from(
        "op2-rs needs your own copy of Outpost 2: Divided Destiny. It reads the game's files from it and includes none \
         of them.\n",
    );
    if let Some(c) = problem {
        s.push_str(&format!("\nThe game folder {} can't be used:\n", c.dir.display()));
        for p in c.problems() {
            s.push_str(&format!("  - {p}\n"));
        }
    } else {
        s.push_str("\nNo Outpost 2 install was found in the usual places.\n");
    }
    s.push_str(&format!(
        "\nOutpost 2 is sold on GOG: {GOG_STORE_URL}\n\
         \nThen either:\n\
         \x20 - point op2-rs at the installed game folder:   op2 setup --game-dir \"<folder>\"\n\
         \x20 - or at GOG's offline installer you downloaded:  op2 setup --installer \"<setup_outpost_2_...exe>\"\n\
         \x20 - or run once with --game-dir <folder>, or set OP2_DIR.\n\
         \nThe choice is saved in settings.toml. More: docs/game-data.md\n"
    ));
    s
}

#[cfg(windows)]
fn registry_candidates() -> Vec<Candidate> {
    win_registry::candidates()
}

#[cfg(not(windows))]
fn registry_candidates() -> Vec<Candidate> {
    Vec::new()
}

#[cfg(windows)]
mod win_registry {
    //! Install folders from the Windows registry: GOG's game key and uninstall entries named Outpost 2.

    use super::{Candidate, GOG_GAME_ID};
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_EXPAND_SZ, REG_SZ,
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0) };
        }
    }

    impl Key {
        fn open(root: HKEY, path: &str, view: u32) -> Option<Key> {
            let mut key: HKEY = std::ptr::null_mut();
            let r = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ | view, &mut key) };
            (r == ERROR_SUCCESS).then_some(Key(key))
        }

        fn string(&self, name: &str) -> Option<String> {
            let name = wide(name);
            let mut kind = 0u32;
            let mut len = 0u32;
            let r = unsafe {
                RegQueryValueExW(self.0, name.as_ptr(), std::ptr::null(), &mut kind, std::ptr::null_mut(), &mut len)
            };
            if r != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) || len == 0 {
                return None;
            }
            let mut buf = vec![0u16; (len as usize).div_ceil(2)];
            let r = unsafe {
                RegQueryValueExW(self.0, name.as_ptr(), std::ptr::null(), &mut kind, buf.as_mut_ptr().cast(), &mut len)
            };
            if r != ERROR_SUCCESS {
                return None;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..end]))
        }

        fn subkeys(&self) -> Vec<String> {
            let mut out = Vec::new();
            for i in 0.. {
                let mut buf = [0u16; 256];
                let mut len = buf.len() as u32;
                let r = unsafe {
                    RegEnumKeyExW(
                        self.0,
                        i,
                        buf.as_mut_ptr(),
                        &mut len,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if r != ERROR_SUCCESS {
                    break;
                }
                out.push(String::from_utf16_lossy(&buf[..len as usize]));
            }
            out
        }
    }

    pub fn candidates() -> Vec<Candidate> {
        let mut out = Vec::new();
        let mut push = |p: String, origin: &str| {
            let p = p.trim().trim_end_matches('\\').to_string();
            if !p.is_empty() {
                out.push(Candidate { dir: PathBuf::from(p), origin: origin.to_string() });
            }
        };
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
                if let Some(k) = Key::open(root, &format!("SOFTWARE\\GOG.com\\Games\\{GOG_GAME_ID}"), view)
                    && let Some(p) = k.string("path")
                {
                    push(p, "GOG's registry key");
                }
                let uninstall = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
                if let Some(k) = Key::open(root, uninstall, view) {
                    for sub in k.subkeys() {
                        let Some(e) = Key::open(root, &format!("{uninstall}\\{sub}"), view) else { continue };
                        if !e.string("DisplayName").is_some_and(|n| n.to_ascii_lowercase().starts_with("outpost 2")) {
                            continue;
                        }
                        if let Some(p) = e.string("InstallLocation").or_else(|| e.string("Inno Setup: App Path")) {
                            push(p, "uninstall entry in the registry");
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder that is removed when dropped.
    struct Tmp(PathBuf);

    impl Tmp {
        fn new(name: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("op2-install-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A fake install: empty files with the names a real one has, spelled the way GOG spells them.
    fn fake_install(dir: &Path, gog: bool) {
        fs::create_dir_all(dir).unwrap();
        for (n, _) in REQUIRED.iter().chain(OPTIONAL) {
            fs::write(dir.join(n), b"").unwrap();
        }
        if gog {
            fs::write(dir.join(format!("goggame-{GOG_GAME_ID}.info")), b"{}").unwrap();
        }
    }

    fn env(home: &Path, os: Os) -> Env {
        Env { os, home: Some(home.to_path_buf()), data_local: Some(home.join("data")), ..Env::default() }
    }

    #[test]
    fn check_reports_missing_files_case_insensitively() {
        let t = Tmp::new("check");
        let dir = t.0.join("Outpost 2");
        fake_install(&dir, true);
        // Renamed to other cases, as a case-sensitive filesystem may hold them.
        fs::rename(dir.join("OP2Shell.dll"), dir.join("op2shell.DLL")).unwrap();
        fs::rename(dir.join("maps.vol"), dir.join("MAPS.VOL")).unwrap();
        let c = check(&dir);
        assert!(c.ok(), "{:?}", c.missing);
        assert_eq!(c.release, Release::Gog);
        assert_eq!(c.exe_matches_gog, Some(false), "an empty file isn't GOG's exe");

        fs::remove_file(dir.join("sheets.vol")).unwrap();
        fs::remove_file(dir.join("e01.dll")).unwrap();
        fs::remove_file(dir.join("op2.clm")).unwrap();
        fs::remove_file(dir.join(format!("goggame-{GOG_GAME_ID}.info"))).unwrap();
        let c = check(&dir);
        assert!(!c.ok());
        assert_eq!(c.missing, vec!["sheets.vol", "e01.dll"]);
        assert_eq!(c.missing_optional, vec!["op2.clm"]);
        assert_eq!(c.release, Release::Cd);
        assert!(c.problems()[0].starts_with("missing sheets.vol"));
        assert_eq!(c.short_problem(), "missing sheets.vol, e01.dll");
        assert_eq!(check(&t.0.join("nowhere")).short_problem(), "no Outpost 2 files in it");
    }

    #[test]
    fn opu_is_detected_with_its_version() {
        let t = Tmp::new("opu");
        // Only the game folder's own OPU: op2-rs may have installed one in this machine's data folder.
        let check = |d: &Path| check_with_opu(d, opu_dir_with(d, None));
        fake_install(&t.0, true);
        assert_eq!(check(&t.0).opu, None);
        fs::create_dir_all(t.0.join("OPU")).unwrap();
        fs::write(t.0.join("OPU/OPUPatch.dll"), b"").unwrap();
        assert_eq!(check(&t.0).opu.as_deref(), Some(""));
        fs::write(t.0.join("OPU/README.md"), "# Outpost 2 OPU Unofficial Update 1.4.x Patch Notes\n").unwrap();
        let c = check(&t.0);
        assert_eq!(c.opu.as_deref(), Some("1.4.x"));
        assert!(c.describe().starts_with("GOG release + OPU 1.4.x"), "{}", c.describe());
        fs::write(t.0.join("OPU/CHANGES.md"), "# Outpost 2 OPU Unofficial Update 1.4.1 Changes\n").unwrap();
        assert_eq!(check(&t.0).summary(), "GOG release + OPU 1.4.1");
    }

    #[test]
    fn opu_is_found_in_the_game_folder_or_op2rs_data_folder() {
        let t = Tmp::new("opu-dirs");
        let game = t.0.join("game");
        let ours = t.0.join("data/OPU");
        fake_install(&game, true);
        assert_eq!(opu_dir_with(&game, Some(&ours)), None, "no OPU anywhere");
        // op2-rs's own install counts once complete (OPUPatch.dll is there), and only for a game folder.
        fs::create_dir_all(&ours).unwrap();
        assert_eq!(opu_dir_with(&game, Some(&ours)), None);
        fs::write(ours.join("OPUPatch.dll"), b"").unwrap();
        assert_eq!(opu_dir_with(&game, Some(&ours)), Some(ours.clone()));
        assert_eq!(opu_dir_with(&t.0, Some(&ours)), None, "not a game folder");
        // The game's own OPU wins, in any case (on Windows, whose file names ignore case, it is found as `OPU`).
        fs::create_dir_all(game.join("opu")).unwrap();
        let found = opu_dir_with(&game, Some(&ours)).map(|p| p.to_string_lossy().to_lowercase());
        assert_eq!(found, Some(game.join("opu").to_string_lossy().to_lowercase()));
    }

    #[test]
    fn opu_plan_asks_only_players_and_never_blocks_test_paths() {
        let t = Tmp::new("opu-plan");
        fake_install(&t.0, true);
        let plain = check(&t.0);
        if plain.opu.is_some() {
            eprintln!("skipping: op2-rs's data folder has an OPU install, which any game folder gets");
            return;
        }
        let found = Origin::Found("GOG default folder".into());
        assert_eq!(opu_plan(&plain, &found, false), if crate::opu::FETCH_BY_DEFAULT { OpuPlan::Fetch } else { OpuPlan::Ask });
        assert_eq!(opu_plan(&plain, &Origin::Settings, false), opu_plan(&plain, &found, false));
        for dev in [Origin::CommandLine, Origin::EnvVar, Origin::SourceTree] {
            assert_eq!(opu_plan(&plain, &dev, false), OpuPlan::Warn, "{dev}");
            assert_eq!(opu_plan(&plain, &dev, true), OpuPlan::Fetch, "asked for");
        }
        assert_eq!(opu_plan(&plain, &found, true), OpuPlan::Fetch);
        fs::create_dir_all(t.0.join("OPU")).unwrap();
        fs::write(t.0.join("OPU/OPUPatch.dll"), b"").unwrap();
        let with = check(&t.0);
        for origin in [found, Origin::CommandLine] {
            assert_eq!(opu_plan(&with, &origin, true), OpuPlan::Ready, "OPU there: no questions, no download");
        }
        assert!(opu_needed_message(&plain).contains(crate::opu::DOWNLOAD_PAGE));
    }

    #[test]
    fn versions_are_found_in_text() {
        assert_eq!(first_version("Update 1.4.x Patch").as_deref(), Some("1.4.x"));
        assert_eq!(first_version("## 1.4.2 (2023)").as_deref(), Some("1.4.2"));
        assert_eq!(first_version("op2ext 3.0 and v12.34.56.").as_deref(), Some("12.34.56"));
        assert_eq!(first_version("version 2 only"), None);
    }

    // A Linux host's Wine prefixes, with Unix paths: not meaningful on a Windows host.
    #[cfg(not(windows))]
    #[test]
    fn finds_gog_install_in_a_wine_prefix_with_odd_case() {
        let t = Tmp::new("wine");
        let home = &t.0;
        fake_install(&home.join(".wine/drive_c/gog games/OUTPOST 2"), true);
        let s = search(&env(home, Os::Linux));
        let (c, chk) = s.best().expect("found");
        // Canonical: Windows ignores case, so the folder is found by the spelling looked for.
        assert!(fs::canonicalize(&c.dir).unwrap().ends_with("gog games/OUTPOST 2"), "{}", c.dir.display());
        assert!(c.origin.contains("GOG default folder"), "{}", c.origin);
        assert!(chk.ok());
    }

    #[test]
    fn registry_in_a_prefix_points_at_a_custom_folder() {
        let t = Tmp::new("reg");
        let prefix = t.0.join("Games/outpost-2");
        fake_install(&prefix.join("drive_c/Games2/OP2"), true);
        fs::write(
            prefix.join("system.reg"),
            "WINE REGISTRY Version 2\n\n[Software\\\\Wow6432Node\\\\GOG.com\\\\Games\\\\1494370738] 1792199325\n\
             \"exe\"=\"C:\\\\Games2\\\\OP2\\\\Outpost2.exe\"\n\"path\"=\"C:\\\\Games2\\\\OP2\"\n\n\
             [Software\\\\Other] 1\n\"path\"=\"C:\\\\nope\"\n",
        )
        .unwrap();
        let s = search(&env(&t.0, Os::Linux));
        let (c, _) = s.best().expect("found");
        assert!(c.dir.ends_with("drive_c/Games2/OP2"), "{}", c.dir.display());
        assert!(c.origin.starts_with("registry of Lutris/Heroic games folder"), "{}", c.origin);
    }

    #[cfg(not(windows))]
    #[test]
    fn reg_file_uninstall_entries() {
        let reg = "[Software\\\\Wow6432Node\\\\Microsoft\\\\Windows\\\\CurrentVersion\\\\Uninstall\\\\1494370738_is1] 1\n\
                   \"DisplayName\"=\"Outpost 2\"\n\"InstallLocation\"=\"D:\\\\Spiele\\\\Outpost 2\\\\\"\n\
                   [Software\\\\Microsoft\\\\Windows\\\\CurrentVersion\\\\Uninstall\\\\Other] 1\n\
                   \"DisplayName\"=\"Something\"\n\"InstallLocation\"=\"C:\\\\x\"\n";
        assert_eq!(registry_paths_in_reg_file(reg), vec!["D:\\Spiele\\Outpost 2\\".to_string()]);
        let p = wine_path(Path::new("/pfx"), "D:\\Spiele\\Outpost 2\\").unwrap();
        assert_eq!(p, Path::new("/pfx/dosdevices/d:/Spiele/Outpost 2"));
        assert_eq!(wine_path(Path::new("/pfx"), "C:\\GOG Games\\Outpost 2").unwrap(), Path::new("/pfx/drive_c/GOG Games/Outpost 2"));
    }

    #[test]
    fn heroic_library_and_extracted_copy() {
        let t = Tmp::new("heroic");
        let home = &t.0;
        let lib = home.join("Spiele/Outpost 2");
        fake_install(&lib, true);
        let cfg = home.join(".config/heroic/gog_store");
        fs::create_dir_all(&cfg).unwrap();
        let json = format!(
            "{{\"installed\":[{{\"platform\":\"windows\",\"install_path\": \"{}\",\"appName\":\"1494370738\"}}]}}",
            lib.display().to_string().replace('\\', "\\\\")
        );
        fs::write(cfg.join("installed.json"), json).unwrap();
        let s = search(&env(home, Os::Linux));
        assert_eq!(s.best().unwrap().0.dir, lib);
        assert_eq!(s.best().unwrap().0.origin, "Heroic's GOG library");

        // op2-rs's own extraction folder is looked at first.
        fake_install(&home.join("data/op2-rs/Outpost 2"), true);
        let s = search(&env(home, Os::Linux));
        assert!(s.best().unwrap().0.origin.starts_with("extracted by op2-rs"));
    }

    #[test]
    fn incomplete_installs_are_reported_but_not_chosen() {
        let t = Tmp::new("incomplete");
        let home = &t.0;
        let first = home.join(".wine/drive_c/GOG Games/Outpost 2");
        fake_install(&first, true);
        fs::remove_file(first.join("maps.vol")).unwrap();
        fake_install(&home.join("GOG Games/Outpost 2"), false);
        let s = search(&env(home, Os::Linux));
        assert_eq!(s.found.len(), 2);
        assert!(!s.found[0].1.ok());
        assert_eq!(s.best().unwrap().0.dir, home.join("GOG Games/Outpost 2"));
        assert_eq!(s.best().unwrap().1.release, Release::Cd);
    }

    #[test]
    fn mac_crossover_bottles_and_windows_defaults_are_listed() {
        let t = Tmp::new("mac");
        let bottle = t.0.join("Library/Application Support/CrossOver/Bottles/Outpost");
        fake_install(&bottle.join("drive_c/GOG Games/Outpost 2"), true);
        let s = search(&env(&t.0, Os::MacOs));
        assert!(s.best().unwrap().0.origin.contains("CrossOver bottle"), "{}", s.best().unwrap().0.origin);

        let mut e = env(&t.0, Os::Windows);
        e.vars.insert("ProgramFiles(x86)".into(), "C:\\Program Files (x86)".into());
        e.registry.push(Candidate { dir: PathBuf::from("E:\\Games\\OP2"), origin: "GOG's registry key".into() });
        let dirs: Vec<String> = known_locations(&e).iter().map(|c| c.dir.display().to_string()).collect();
        let pos = |s: &str| dirs.iter().position(|d| d.replace("\\/", "\\").replace('/', "\\") == s).unwrap_or_else(|| panic!("{s} in {dirs:?}"));
        assert!(pos("E:\\Games\\OP2") < pos("C:\\GOG Games\\Outpost 2"));
        pos("C:\\Program Files (x86)\\GOG Galaxy\\Games\\Outpost 2");
        pos("C:\\Sierra\\Outpost 2");
        pos("D:\\GOG Games\\Outpost 2");
    }

    #[test]
    fn choosing_a_folder_above_the_install_finds_it() {
        let t = Tmp::new("chosen");
        let prefix = t.0.join("pfx");
        fake_install(&prefix.join("drive_c/GOG Games/Outpost 2"), true);
        assert!(check_chosen(&prefix).dir.ends_with("drive_c/GOG Games/Outpost 2"));
        assert!(check_chosen(&prefix.join("drive_c/GOG Games")).ok());
        let empty = t.0.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let c = check_chosen(&empty);
        assert_eq!((c.dir.as_path(), c.ok()), (empty.as_path(), false));
    }

    #[test]
    fn json_values() {
        let v = json_string_values(r#"[{"install_path":"C:\\Games\\a \"b\"","x":1},{"install_path" : "/h/é\u00e9"}]"#, "install_path");
        assert_eq!(v, vec!["C:\\Games\\a \"b\"".to_string(), "/h/éé".to_string()]);
    }

    #[test]
    fn real_install_is_recognised() {
        let Some(dir) = crate::testdata::game_dir() else { return };
        let c = check(&dir);
        assert!(c.ok(), "{:?}", c.missing);
        // The project's install is GOG's; other installs only need to pass the check.
        if c.release == Release::Gog {
            assert_eq!(c.exe_matches_gog, Some(true), "{}", c.describe());
            assert_eq!(c.exe_version.as_deref(), Some("1.2.0.7"));
        }
    }
}
