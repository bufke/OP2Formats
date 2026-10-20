//! The OPU community update ("OPU Unofficial Update" by the Outpost Universe community, https://outpost2.net), which
//! op2-rs needs: REQUIREMENTS.md's target is GOG 1.2 + OPU 1.4.1, and OPU replaces much of the game's data (rule
//! sheets, tech trees, patched missions, 40+ community missions, art). GOG's installer doesn't include it.
//!
//! OPU is free but licensed CC BY-NC-ND 4.0 (its `LICENSE.txt`): it may be passed on unchanged, non-commercially,
//! with credit, and the Outpost Universe asks people to get it from outpost2.net. So op2-rs never ships it. The
//! player downloads `OPUInstaller-1.4.1.exe` from the official page (or op2-rs fetches that exact file, pinned by
//! SHA-256, when asked to), and op2-rs installs it into its own data folder ([`crate::install::opu_install_dir`]),
//! never touching the player's game folder.
//!
//! Installing does what OPU's own installer does on a GOG 1.2 game, without running it:
//!
//! 1. The installer is an NSIS 3 installer ([`crate::nsis`]). Its OPU files (`OPU/README.md`, `LICENSE.txt`,
//!    `OPUPatch.dll`, `base/ui`, `base/language.ini`, `libs`, the community missions in `maps`) are copied out unchanged.
//! 2. Its helper plugin (`$PLUGINSDIR\Helper.dll`, OPU's `ExtractGameFiles`) copies the player's own game files into
//!    `OPU/base`: every entry of `sound.vol`, `voices.vol`, `story.vol` and `sheets.vol`, the tilesets and sprites of
//!    `maps.vol`, and each mission's DLLs and maps into `base/maps/<mission>`. The layout below was read from the
//!    plugin's tables (strings near "/OPU/base") and checked against a tree OPU's installer made.
//!    Tilesets stored in the game's "PBMP" container are written as plain BMPs, as OPU's installer does.
//! 3. The plugin's `ApplyPatchSet "1400"` then patches some of those copies (sheets, tech trees, campaign and colony
//!    maps). The patches are a zip appended to `Helper.dll`, one file per change named `<old SHA-1>_<new SHA-1>.dat`:
//!    a bsdiff control/diff/extra stream (Matthew Endsley's headerless variant) with every byte XORed with 0xAA. A
//!    copy whose SHA-1 names a patch gets it, and the result must hash to the patch's new SHA-1.
//!
//! Not done (op2-rs doesn't use them): the root `op2ext.dll`/`winmm.dll` loader, `OPU/outpost2.ini`, the CD-only steps
//! (Sierra's update installers, localised files from a CD, music extraction) and the installer's backups.

use std::path::{Path, PathBuf};

/// The OPU release op2-rs is built and measured against (every oracle recording is of it).
pub const VERSION: &str = "1.4.1";
/// OPU's official download page, where players are sent to get it.
pub const DOWNLOAD_PAGE: &str = "https://outpost2.net/outpost2.html";
/// The official 1.4.1 installer, linked from [`DOWNLOAD_PAGE`] ("OPU 1.4.1 Unofficial Update (for CD or GOG
/// installation)"; served since 2021-02-08).
pub const PACKAGE_URL: &str = "https://outpost2.net/files/op2/game/OPUInstaller-1.4.1.exe";
pub const PACKAGE_NAME: &str = "OPUInstaller-1.4.1.exe";
pub const PACKAGE_SIZE: u64 = 14_035_039;
pub const PACKAGE_SHA256: &str = "14f534719e97bf747c6facb090f147fa4c44278605b97ae246d4ee778c58ee55";
/// Who made OPU and under what terms, for every place op2-rs mentions or installs it.
pub const CREDIT: &str = "OPU Unofficial Update by the Outpost Universe community (outpost2.net), \
                          licensed CC BY-NC-ND 4.0";
/// Download OPU without asking when it's missing. Off until the OPU maintainers agree; turning it on makes the
/// engine and `op2 setup` fetch it as if `--fetch-opu` were given.
pub const FETCH_BY_DEFAULT: bool = false;

/// Where a step of the install is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Downloading the installer: bytes so far and in total.
    Download,
    /// Checking and unpacking the installer.
    Read,
    /// Writing files: files so far and in total.
    Install,
}

/// What [`install`] did.
#[derive(Debug)]
pub struct Installed {
    pub dir: PathBuf,
    pub files: usize,
    /// Things worth telling the player (the result still works).
    pub warnings: Vec<String>,
}

#[cfg(feature = "opu")]
pub use imp::*;

#[cfg(feature = "opu")]
mod imp {
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::io::Read;
    use std::path::{Path, PathBuf};

    use super::{Installed, PACKAGE_SHA256, Stage, VERSION};
    use crate::{Error, VolArchive};

    /// OPU's `base/maps` folders and the game files each gets (OPU's Helper.dll `ExtractGameFiles` table).
    /// Mission DLLs come from the game folder, maps from it or from `maps.vol`.
    const MAP_FOLDERS: &[(&str, &[&str])] = &[
        ("AlliedSiege", &["ml6_03.dll", "mp6_03.map"]),
        ("AmbushAlley", &["ml2_08.dll", "mp2_08.map"]),
        ("AroundTheWorld", &["mf6_02.dll", "ml6_02.dll", "mu6_02.dll", "newworld.map"]),
        ("AxensHome", &["ml6_01.dll", "mp6_01.map", "mr6_01.dll", "mu6_01.dll"]),
        ("BarrenLand", &["mm4_04.dll", "mp4_04.map", "mu4_04.dll"]),
        ("BeachParty", &["ml2_10.dll", "mp2_10.map", "mr2_10.dll", "mu2_10.dll"]),
        ("Biohazard", &["mf3_02.dll", "mp3_02.map"]),
        ("campaign", &[
            "e01.dll", "e02.dll", "e03.dll", "e04.dll", "e05.dll", "e06.dll", "e07.dll", "e08.dll", "e09.dll", "e10.dll",
            "e11.dll", "e12.dll", "eden01.map", "eden02.map", "eden03.map", "eden04.map", "eden05.map", "eden06.map",
            "eden07.map", "eden08.map", "eden09.map", "eden10.map", "eden11.map", "eden12.map", "p01.dll", "p02.dll",
            "p03.dll", "p04.dll", "p05.dll", "p06.dll", "p07.dll", "p08.dll", "p09.dll", "p10.dll", "p11.dll",
            "p12.dll", "ply01.map", "ply02.map", "ply03.map", "ply05.map", "ply06.map", "ply07.map", "ply08.map",
            "ply09.map", "ply11.map", "ply12.map",
        ]),
        ("CloseEncounter", &["mf2_01.dll", "ml2_01.dll", "mp2_01.map"]),
        ("Coliseum", &["ml4_01.dll", "mm4_01.dll", "mp4_01.map"]),
        ("ColonyEdenPopulation", &["CEP1.dll", "cm02.map"]),
        ("ColonyEdenStarship", &["CES1.dll", "cm03.map"]),
        ("ColonyPlymouthPopulation", &["cm04.map", "CPP1.dll"]),
        ("ColonyPlymouthStarship", &["cm01.map", "CPS1.dll"]),
        ("ColonyPlymouthStarship2", &["cps2.dll", "on1_01.map"]),
        ("CrashZone", &["mf4_05.dll", "ml4_05.dll", "mp4_05.map", "mr4_05.dll"]),
        ("DoubleCross", &["ml4_10.dll", "mp4_10.map", "mr4_10.dll"]),
        ("Earlobe", &["ml6_31.dll", "on6_03.map"]),
        ("FloodPlain", &["mu4_31.dll", "on4_02.map"]),
        ("FourMesas", &["ml4_07.dll", "mp4_07.map", "mu4_07.dll"]),
        ("Fractures", &["mu6_41.dll", "on6_04.map"]),
        ("GreatDivide", &["ml2_02.dll", "mp2_02.map", "mu2_02.dll"]),
        ("Handprint", &["ml4_41.dll", "on4_03.map"]),
        ("HiddenTreasure", &["mm4_11.dll", "mp4_11.map"]),
        ("Labyrinth", &["ml2_11.dll", "mp2_11.map"]),
        ("LaCorrida", &["ml4_21.dll", "on4_01.map"]),
        ("misc", &["ademo1.dll", "ademo2.dll", "ademo3.dll", "mp3_01.map", "mp4_06.map", "mp4_09.map", "us_them.dll"]),
        ("OlympusMons", &["ml4_12.dll", "mp4_12.map", "mr4_12.dll"]),
        ("Peacekeeper", &["ml2_21.dll", "on2_01.map"]),
        ("PieChart", &["ml6_21.dll", "on6_01.map"]),
        ("RiverOfFire", &["ml2_03.dll", "mp2_03.map"]),
        ("RockGarden", &["mf4_03.dll", "mp4_03.map", "mr4_03.dll", "mu4_03.dll"]),
        ("RootCanal", &["mf6_12.dll", "ml6_12.dll", "mp2_12.map", "mu6_12.dll"]),
        ("TheRift", &["mf4_08.dll", "ml4_08.dll", "mm4_08.dll", "mp4_08.map", "mu4_08.dll"]),
        ("ThreesACrowd", &["mf3_08.dll", "ml3_08.dll", "mm3_08.dll", "mp3_08.map", "mr3_08.dll", "mu3_08.dll"]),
        ("TugOWar", &["ml2_31.dll", "on2_02.map"]),
        ("tutorial", &[
            "t01.dll", "t02.dll", "t03.dll", "t04.dll", "t05.dll", "t06.dll", "t07.dll", "t08.dll", "t09.dll", "t10.dll",
            "t11.dll", "t12.dll", "t13.dll", "t14.dll", "tutorial.map",
        ]),
        ("TwinValleys", &["mm2_07.dll", "mp2_07.map"]),
        ("UnsettledEarth", &["ml4_13.dll", "mpdemo.map", "mu4_13.dll"]),
        ("Wishbone", &["ml2_05.dll", "mp2_05.map"]),
    ];

    /// `maps.vol` entries that aren't maps: tilesets and the sprite files.
    const SPRITES: &[&str] = &["color.bmp", "op2_art.prt", "virmask.raw"];

    /// SHA-256 over the sorted `path<TAB>sha1` lines of the tree [`install`] writes from OPU 1.4.1's installer and
    /// GOG's Outpost 2 1.2 (setup_outpost_2_1.2_(37379).exe): the same files OPU's own installer puts in `OPU`.
    /// Checked on a fresh GOG extraction and on two OPU-installed GOG games (665 files, all identical to the ones
    /// OPU's installer wrote).
    pub const GOG_TREE_SHA256: &str = "caaf0c9fc35b75084ede72f1e8c82f814ce498095e4797658b845567ed1a6514";

    /// The OPU folder of an install in `dir` (the folder itself or its `OPU` subfolder), if it has OPU's files.
    pub fn find_opu_folder(dir: &Path) -> Option<PathBuf> {
        let has_patch = |d: &Path| crate::find_in_dir(d, "OPUPatch.dll").is_ok();
        if has_patch(dir) {
            return Some(dir.to_path_buf());
        }
        let sub = crate::install::resolve_ci(dir, Path::new("OPU"))?;
        has_patch(&sub).then_some(sub)
    }

    /// Installs OPU from `package` (OPU 1.4.1's installer, or an installed OPU folder to copy) into `target`, using
    /// the game in `game` for the game files OPU's installer copies and patches. Goes to `<target>.partial` first and
    /// replaces `target` only when complete.
    pub fn install(package: &Path, game: &Path, target: &Path, progress: &mut dyn FnMut(Stage, u64, u64)) -> Result<Installed, Error> {
        let io = |p: &Path, e: std::io::Error| Error::Io { path: p.display().to_string(), source: e };
        let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "OPU".into());
        let partial = target.with_file_name(format!("{name}.partial"));
        if partial.exists() {
            fs::remove_dir_all(&partial).map_err(|e| io(&partial, e))?;
        }
        fs::create_dir_all(&partial).map_err(|e| io(&partial, e))?;
        let result = if package.is_dir() {
            copy_folder(package, &partial, progress)
        } else {
            fs::read(package).map_err(|e| io(package, e)).and_then(|exe| install_package(&exe, game, &partial, progress))
        };
        let (files, warnings) = match result {
            Ok(r) => r,
            Err(e) => {
                let _ = fs::remove_dir_all(&partial);
                return Err(e);
            }
        };
        if target.exists() {
            fs::remove_dir_all(target).map_err(|e| io(target, e))?;
        }
        fs::rename(&partial, target).map_err(|e| io(target, e))?;
        Ok(Installed { dir: target.to_path_buf(), files, warnings })
    }

    /// Checks `exe` is the pinned OPU installer.
    pub fn verify_package(exe: &[u8]) -> Result<(), Error> {
        let got = crate::sha256::hex(exe);
        if got != PACKAGE_SHA256 {
            return Err(Error::Invalid(format!(
                "this isn't OPU {VERSION}'s installer ({} expected, SHA-256 {PACKAGE_SHA256}; this file's is {got})",
                super::PACKAGE_NAME
            )));
        }
        Ok(())
    }

    /// The OPU tree from the installer and the game's files into `out`: (files written, warnings).
    fn install_package(exe: &[u8], game: &Path, out: &Path, progress: &mut dyn FnMut(Stage, u64, u64)) -> Result<(usize, Vec<String>), Error> {
        progress(Stage::Read, 0, 1);
        verify_package(exe)?;
        let nsis = crate::nsis::Nsis::parse(exe)?;
        let helper = nsis
            .files()
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case("$PLUGINSDIR\\Helper.dll"))
            .ok_or_else(|| Error::Invalid("the OPU installer has no Helper.dll".into()))?;
        let patches = patches(&nsis.read(helper)?)?;
        // OPU's own files: everything the script puts under $INSTDIR\OPU whose path it fully knows (the localised
        // `lang\$LANG` files are for CD installs).
        let mut own: BTreeMap<String, &crate::nsis::NsisFile> = BTreeMap::new();
        for f in nsis.files() {
            if let Some(rel) = strip_prefix_ci(&f.path, "$INSTDIR\\OPU\\")
                && !rel.contains('$')
            {
                own.insert(rel.replace('\\', "/"), f);
            }
        }
        let base = game_files(game)?;
        let total = (base.len() + own.len()) as u64;
        let mut done = 0u64;
        let mut warnings = Vec::new();
        for (rel, data) in base {
            let data = patch(data, &patches).map_err(|e| Error::Invalid(format!("OPU/{rel}: {e}")))?;
            write(out, &rel, &data)?;
            done += 1;
            progress(Stage::Install, done, total);
        }
        // Written after the game's copies, as OPU's installer does: its files win.
        for (rel, f) in &own {
            write(out, rel, &nsis.read(f)?)?;
            done += 1;
            progress(Stage::Install, done, total);
        }
        let digest = tree_digest(out)?;
        if digest != GOG_TREE_SHA256 {
            warnings.push(format!(
                "the installed OPU differs from what OPU's installer makes from GOG's Outpost 2 1.2 (tree {digest}); \
                 the game folder's files may not be GOG's"
            ));
        }
        Ok((done as usize, warnings))
    }

    /// OPU's `ExtractGameFiles`: `(path below OPU, contents)` for each game file it copies into `OPU/base`, sorted.
    fn game_files(game: &Path) -> Result<BTreeMap<String, Vec<u8>>, Error> {
        let open_vol = |name: &str| -> Result<Option<VolArchive>, Error> {
            let Ok(path) = crate::find_in_dir(game, name) else { return Ok(None) };
            Ok(Some(VolArchive::open(&path)?))
        };
        let folder_of_map: HashMap<String, &str> = MAP_FOLDERS
            .iter()
            .flat_map(|(folder, names)| names.iter().map(move |n| (n.to_ascii_lowercase(), *folder)))
            .collect();
        let mut out = BTreeMap::new();
        for (vol_name, folder) in [("sound.vol", "sounds"), ("voices.vol", "voices"), ("story.vol", "story"), ("sheets.vol", ""), ("maps.vol", "")] {
            let Some(vol) = open_vol(vol_name)? else { continue };
            for e in vol.entries() {
                let lower = e.name.to_ascii_lowercase();
                let folder = match (vol_name, folder) {
                    (_, f) if !f.is_empty() => f.to_string(),
                    ("sheets.vol", _) if lower.ends_with("tek.txt") => "techs".into(),
                    ("sheets.vol", _) => "sheets".into(),
                    _ if lower.ends_with(".map") => match folder_of_map.get(&lower) {
                        Some(f) => format!("maps/{f}"),
                        None => continue,
                    },
                    _ if lower.starts_with("well") || lower == "blue.bmp" => "tilesets".into(),
                    _ if SPRITES.contains(&lower.as_str()) => "sprites".into(),
                    _ => continue,
                };
                let mut data = vol.data(e)?.into_owned();
                if folder == "tilesets" && data.starts_with(b"PBMP") {
                    data = crate::tileset::pbmp_to_bmp(&data)?;
                }
                out.insert(format!("base/{folder}/{}", e.name), data);
            }
        }
        // Mission DLLs (and the maps that ship loose, like on4_01.map) come from the game folder, which wins over a
        // map of the same name in maps.vol, as in the game.
        for (folder, names) in MAP_FOLDERS {
            for name in *names {
                let Ok(path) = crate::find_in_dir(game, name) else { continue };
                let data = fs::read(&path).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?;
                out.retain(|k: &String, _| !k.eq_ignore_ascii_case(&format!("base/maps/{folder}/{name}")));
                out.insert(format!("base/maps/{folder}/{name}"), data);
            }
        }
        Ok(out)
    }

    /// OPU's patches, by the SHA-1 of the file each applies to: (SHA-1 of the result, patch stream).
    fn patches(helper: &[u8]) -> Result<HashMap<String, (String, Vec<u8>)>, Error> {
        let mut out = HashMap::new();
        for (name, data) in zip_entries(helper)? {
            let Some((from, to)) = name.strip_suffix(".dat").and_then(|n| n.split_once('_')) else { continue };
            if from.len() == 40 && to.len() == 40 {
                out.insert(from.to_ascii_lowercase(), (to.to_ascii_lowercase(), data));
            }
        }
        if out.is_empty() {
            return Err(Error::Invalid("OPU's Helper.dll carries no patches".into()));
        }
        Ok(out)
    }

    /// `data` with OPU's patch for it applied, if it has one; the result must hash to what the patch's name says.
    fn patch(data: Vec<u8>, patches: &HashMap<String, (String, Vec<u8>)>) -> Result<Vec<u8>, Error> {
        let sha = sha1_smol::Sha1::from(&data).digest().to_string();
        let Some((to, stream)) = patches.get(&sha) else { return Ok(data) };
        let new = bspatch(&data, stream)?;
        let got = sha1_smol::Sha1::from(&new).digest().to_string();
        if &got != to {
            return Err(Error::Invalid(format!("OPU's patch {sha}_{to} gave {got}")));
        }
        Ok(new)
    }

    /// Applies a bsdiff stream in OPU's form: no header, every byte XORed with 0xAA, then repeated (diff length,
    /// extra length, seek) control triples, each followed by its diff bytes (added to the old file) and extra bytes.
    pub(crate) fn bspatch(old: &[u8], patch: &[u8]) -> Result<Vec<u8>, Error> {
        let p: Vec<u8> = patch.iter().map(|b| b ^ 0xAA).collect();
        let bad = || Error::Invalid("corrupt OPU patch".into());
        // bsdiff's sign-and-magnitude integers.
        let int = |b: &[u8]| {
            let v = i64::from_le_bytes(b.try_into().unwrap()) & i64::MAX;
            if b[7] & 0x80 != 0 { -v } else { v }
        };
        let mut new = Vec::new();
        let (mut pos, mut old_pos) = (0usize, 0i64);
        while pos < p.len() {
            let ctrl = p.get(pos..pos + 24).ok_or_else(bad)?;
            let (add, copy, seek) = (int(&ctrl[..8]), int(&ctrl[8..16]), int(&ctrl[16..]));
            pos += 24;
            let (add, copy) = (usize::try_from(add).map_err(|_| bad())?, usize::try_from(copy).map_err(|_| bad())?);
            let diff = p.get(pos..pos.checked_add(add).ok_or_else(bad)?).ok_or_else(bad)?;
            for (i, d) in diff.iter().enumerate() {
                let o = old_pos + i as i64;
                let base = if o >= 0 && (o as usize) < old.len() { old[o as usize] } else { 0 };
                new.push(d.wrapping_add(base));
            }
            pos += add;
            old_pos += add as i64;
            new.extend_from_slice(p.get(pos..pos.checked_add(copy).ok_or_else(bad)?).ok_or_else(bad)?);
            pos += copy;
            old_pos += seek;
        }
        Ok(new)
    }

    /// The files of a zip archive that may have other data in front of it (Helper.dll carries one at its end):
    /// stored and deflated entries, each checked against its CRC-32.
    pub(crate) fn zip_entries(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Error> {
        let bad = |what: &str| Error::Invalid(format!("bad zip archive: {what}"));
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(|| bad("truncated"));
        let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize).ok_or_else(|| bad("truncated"));
        let eocd = (0..data.len().saturating_sub(21)).rev().find(|&o| data[o..].starts_with(b"PK\x05\x06")).ok_or_else(|| bad("no end record"))?;
        let count = u16_at(eocd + 10)?;
        let cd_size = u32_at(eocd + 12)?;
        let cd_offset = u32_at(eocd + 16)?;
        // Offsets are from the start of the archive, which isn't the start of the file when something precedes it.
        let shift = eocd.checked_sub(cd_offset + cd_size).ok_or_else(|| bad("directory out of range"))?;
        let mut p = cd_offset + shift;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if u32_at(p)? != 0x0201_4b50 {
                return Err(bad("directory entry"));
            }
            let (method, crc, csize, usize_) = (u16_at(p + 10)?, u32_at(p + 16)? as u32, u32_at(p + 20)?, u32_at(p + 24)?);
            let (nlen, elen, clen) = (u16_at(p + 28)?, u16_at(p + 30)?, u16_at(p + 32)?);
            let local = u32_at(p + 42)? + shift;
            let name = String::from_utf8_lossy(data.get(p + 46..p + 46 + nlen).ok_or_else(|| bad("name"))?).into_owned();
            p += 46 + nlen + elen + clen;
            if u32_at(local)? != 0x0403_4b50 {
                return Err(bad("local header"));
            }
            let start = local + 30 + u16_at(local + 26)? + u16_at(local + 28)?;
            let raw = data.get(start..start + csize).ok_or_else(|| bad("data out of range"))?;
            let body = match method {
                0 => raw.to_vec(),
                8 => {
                    let mut v = Vec::with_capacity(usize_);
                    flate2::read::DeflateDecoder::new(raw).read_to_end(&mut v).map_err(|e| bad(&e.to_string()))?;
                    v
                }
                m => return Err(Error::Unsupported(format!("zip compression method {m} in {name}"))),
            };
            if body.len() != usize_ || crc32fast::hash(&body) != crc {
                return Err(bad(&format!("{name} fails its CRC")));
            }
            if !name.ends_with('/') {
                out.push((name, body));
            }
        }
        Ok(out)
    }

    /// Copies an installed OPU folder (`src` or its `OPU` subfolder), leaving out what the game and the player made
    /// there (saves, logs, settings, the installer's backups).
    fn copy_folder(src: &Path, out: &Path, progress: &mut dyn FnMut(Stage, u64, u64)) -> Result<(usize, Vec<String>), Error> {
        let src = find_opu_folder(src)
            .ok_or_else(|| Error::Invalid(format!("{} isn't an OPU folder (no OPUPatch.dll)", src.display())))?;
        let skip = |rel: &str| {
            let top = rel.split('/').next().unwrap_or("").to_ascii_lowercase();
            ["saves", "backups", "harness", "outpost2log.txt"].contains(&top.as_str()) || top.starts_with("outpost2.ini")
        };
        let mut files = Vec::new();
        list_files(&src, "", &mut files)?;
        files.retain(|r| !skip(r));
        let total = files.len() as u64;
        for (i, rel) in files.iter().enumerate() {
            let from = src.join(rel);
            let data = fs::read(&from).map_err(|e| Error::Io { path: from.display().to_string(), source: e })?;
            write(out, rel, &data)?;
            progress(Stage::Install, i as u64 + 1, total);
        }
        let mut warnings = Vec::new();
        let version = crate::install::opu_version_of(&src).unwrap_or_default();
        if version != VERSION {
            warnings.push(format!("this OPU is version {version:?}; op2-rs is made for OPU {VERSION}"));
        }
        Ok((files.len(), warnings))
    }

    /// Paths of every file below `dir`, relative, with `/` separators, sorted.
    fn list_files(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), Error> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|e| Error::Io { path: dir.display().to_string(), source: e })?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            if e.path().is_dir() {
                list_files(&e.path(), &r, out)?;
            } else {
                out.push(r);
            }
        }
        Ok(())
    }

    /// SHA-256 over the sorted `path<TAB>sha1` lines of the files below `dir`.
    pub fn tree_digest(dir: &Path) -> Result<String, Error> {
        let mut files = Vec::new();
        list_files(dir, "", &mut files)?;
        files.sort();
        let mut h = crate::sha256::Sha256::new();
        for rel in files {
            let data = fs::read(dir.join(&rel)).map_err(|e| Error::Io { path: rel.clone(), source: e })?;
            h.update(format!("{rel}\t{}\n", sha1_smol::Sha1::from(&data).digest()).as_bytes());
        }
        Ok(h.hex())
    }

    fn write(root: &Path, rel: &str, data: &[u8]) -> Result<(), Error> {
        let path = rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::Io { path: parent.display().to_string(), source: e })?;
        }
        fs::write(&path, data).map_err(|e| Error::Io { path: path.display().to_string(), source: e })
    }

    fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
        (s.len() >= prefix.len() && s.is_char_boundary(prefix.len()) && s[..prefix.len()].eq_ignore_ascii_case(prefix))
            .then(|| &s[prefix.len()..])
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// bsdiff's sign-and-magnitude integer.
        fn int(v: i64) -> [u8; 8] {
            let mut b = v.unsigned_abs().to_le_bytes();
            if v < 0 {
                b[7] |= 0x80;
            }
            b
        }

        /// A patch in OPU's form from (diff bytes, extra bytes, seek) blocks.
        fn opu_patch(blocks: &[(&[u8], &[u8], i64)]) -> Vec<u8> {
            let mut p = Vec::new();
            for (diff, extra, seek) in blocks {
                p.extend(int(diff.len() as i64));
                p.extend(int(extra.len() as i64));
                p.extend(int(*seek));
                p.extend(*diff);
                p.extend(*extra);
            }
            p.iter().map(|b| b ^ 0xAA).collect()
        }

        #[test]
        fn bspatch_adds_diffs_copies_extras_and_seeks() {
            let old = b"0123456789";
            // new = "1234" (old[1..5] + 0) + "xy", then seek back to old[0] and add 1 to "01" -> "12".
            let patch = opu_patch(&[(&[0, 0, 0, 0], b"xy", -5), (&[1, 1], b"", 0)]);
            let mut p = opu_patch(&[(&[], b"", 1)]);
            p.extend(patch);
            assert_eq!(bspatch(old, &p).unwrap(), b"1234xy12");
            // Reading past the old file adds nothing; a truncated patch is an error.
            assert_eq!(bspatch(b"", &opu_patch(&[(&[5], b"", 0)])).unwrap(), [5]);
            let p = opu_patch(&[(&[0, 0], b"z", 0)]);
            assert!(bspatch(old, &p[..p.len() - 1]).is_err());
        }

        #[test]
        fn patches_apply_only_with_the_right_result() {
            let old = b"original".to_vec();
            let new = b"OPU file".to_vec();
            let sha = |d: &[u8]| sha1_smol::Sha1::from(d).digest().to_string();
            let diff: Vec<u8> = new.iter().zip(&old).map(|(n, o)| n.wrapping_sub(*o)).collect();
            let stream = opu_patch(&[(&diff, b"", 0)]);
            let mut set = HashMap::new();
            set.insert(sha(&old), (sha(&new), stream.clone()));
            assert_eq!(patch(old.clone(), &set).unwrap(), new);
            assert_eq!(patch(b"other".to_vec(), &set).unwrap(), b"other", "no patch for it");
            set.insert(sha(&old), ("0".repeat(40), stream));
            assert!(patch(old, &set).is_err(), "a result that doesn't match the name is refused");
        }

        #[test]
        fn zip_after_other_data() {
            use std::io::Write;
            let files: [(&str, &[u8], u16); 2] = [("a_b.dat", b"hello hello hello", 8), ("dir/c.txt", b"stored", 0)];
            let mut zip = b"MZ some executable".to_vec();
            let start = zip.len();
            let mut central = Vec::new();
            for (name, data, method) in files {
                let body = if method == 8 {
                    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                    e.write_all(data).unwrap();
                    e.finish().unwrap()
                } else {
                    data.to_vec()
                };
                let crc = crc32fast::hash(data);
                let local = (zip.len() - start) as u32;
                let fields = |sig: u32, extra_head: &[u8]| {
                    let mut h = sig.to_le_bytes().to_vec();
                    h.extend(extra_head);
                    h.extend([20, 0, 0, 0]);
                    h.extend(method.to_le_bytes());
                    h.extend([0; 4]);
                    h.extend(crc.to_le_bytes());
                    h.extend((body.len() as u32).to_le_bytes());
                    h.extend((data.len() as u32).to_le_bytes());
                    h.extend((name.len() as u16).to_le_bytes());
                    h.extend([0, 0]);
                    h
                };
                zip.extend(fields(0x0403_4b50, &[]));
                zip.extend(name.as_bytes());
                zip.extend(&body);
                let mut c = fields(0x0201_4b50, &[20, 0]);
                c.extend([0; 10]);
                c.extend(local.to_le_bytes());
                c.extend(name.as_bytes());
                central.push(c);
            }
            let cd_offset = (zip.len() - start) as u32;
            let cd: Vec<u8> = central.concat();
            zip.extend(&cd);
            zip.extend(0x0605_4b50u32.to_le_bytes());
            zip.extend([0; 4]);
            zip.extend(2u16.to_le_bytes());
            zip.extend(2u16.to_le_bytes());
            zip.extend((cd.len() as u32).to_le_bytes());
            zip.extend(cd_offset.to_le_bytes());
            zip.extend([0, 0]);
            let got = zip_entries(&zip).unwrap();
            assert_eq!(got, vec![("a_b.dat".to_string(), files[0].1.to_vec()), ("dir/c.txt".to_string(), b"stored".to_vec())]);
            // A damaged entry fails its CRC.
            let i = zip.windows(6).position(|w| w == b"stored").unwrap();
            zip[i] = b'S';
            assert!(zip_entries(&zip).is_err());
        }

        #[test]
        fn copying_an_opu_folder_leaves_out_saves_and_settings() {
            let t = std::env::temp_dir().join(format!("op2-opu-copy-{}", std::process::id()));
            let _ = fs::remove_dir_all(&t);
            let game = t.join("game");
            for f in ["OPU/OPUPatch.dll", "OPU/CHANGES.md", "OPU/base/sheets/vehicles.txt", "OPU/saves/s.op2", "OPU/outpost2.ini", "Outpost2.exe"] {
                write(&game, f, b"x").unwrap();
            }
            write(&game, "OPU/CHANGES.md", b"# Outpost 2 OPU Unofficial Update 1.4.1 Changes\n").unwrap();
            // The game folder or its OPU folder may be chosen.
            for src in [game.clone(), game.join("OPU")] {
                let done = install(&src, &game, &t.join("OPU"), &mut |_, _, _| {}).unwrap();
                assert!(done.warnings.is_empty(), "{:?}", done.warnings);
                let mut files = Vec::new();
                list_files(&t.join("OPU"), "", &mut files).unwrap();
                assert_eq!(files, ["CHANGES.md", "OPUPatch.dll", "base/sheets/vehicles.txt"]);
            }
            assert!(install(&t.join("OPU/base"), &game, &t.join("x"), &mut |_, _, _| {}).is_err());
            let _ = fs::remove_dir_all(&t);
        }
    }
}

#[cfg(feature = "fetch-opu")]
pub use fetch::download;

#[cfg(feature = "fetch-opu")]
mod fetch {
    use std::fs;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};

    use super::{PACKAGE_NAME, PACKAGE_SHA256, PACKAGE_SIZE, PACKAGE_URL, Stage};
    use crate::Error;

    /// Downloads OPU's installer from [`PACKAGE_URL`] into `dir` and checks it against [`PACKAGE_SHA256`]. Returns
    /// the file's path; nothing is left behind on failure.
    pub fn download(dir: &Path, progress: &mut dyn FnMut(Stage, u64, u64)) -> Result<PathBuf, Error> {
        let io = |p: &Path, e: std::io::Error| Error::Io { path: p.display().to_string(), source: e };
        fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        let path = dir.join(PACKAGE_NAME);
        let partial = dir.join(format!("{PACKAGE_NAME}.partial"));
        let result = fetch_to(&partial, progress);
        if let Err(e) = result {
            let _ = fs::remove_file(&partial);
            return Err(e);
        }
        fs::rename(&partial, &path).map_err(|e| io(&path, e))?;
        Ok(path)
    }

    fn fetch_to(path: &Path, progress: &mut dyn FnMut(Stage, u64, u64)) -> Result<(), Error> {
        let net = |e: ureq::Error| Error::Invalid(format!("downloading {PACKAGE_URL}: {e}"));
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(std::time::Duration::from_secs(20)))
            .timeout_recv_body(Some(std::time::Duration::from_secs(300)))
            .user_agent(concat!("op2-rs/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        let response = agent.get(PACKAGE_URL).call().map_err(net)?;
        let mut body = response.into_body();
        let mut reader = body.with_config().limit(PACKAGE_SIZE + 1).reader();
        let mut file = fs::File::create(path).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?;
        let mut hash = crate::sha256::Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut got = 0u64;
        progress(Stage::Download, 0, PACKAGE_SIZE);
        loop {
            let n = reader.read(&mut buf).map_err(|e| Error::Invalid(format!("downloading {PACKAGE_URL}: {e}")))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?;
            got += n as u64;
            progress(Stage::Download, got.min(PACKAGE_SIZE), PACKAGE_SIZE);
        }
        let sha = hash.hex();
        if got != PACKAGE_SIZE || sha != PACKAGE_SHA256 {
            return Err(Error::Invalid(format!(
                "the file downloaded from {PACKAGE_URL} isn't OPU {}'s installer ({got} bytes, SHA-256 {sha}); \
                 nothing was installed",
                super::VERSION
            )));
        }
        Ok(())
    }
}

/// Where op2-rs downloads OPU's installer to before installing it.
pub fn download_dir(data: &Path) -> PathBuf {
    data.join("downloads")
}
