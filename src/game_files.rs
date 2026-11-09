//! Finds game files by name the way an OPU install layers them: loose files under `OPU/base`
//! override `OPU/maps`, which override the game folder, which overrides `.vol` archives. The OPU folder is the game
//! folder's own, or the one op2-rs installed in its data folder ([`crate::install::opu_dir`]).
//! Lookups are case-insensitive, matching Windows.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Error, VolArchive};

/// Where a resolved file came from.
#[derive(Debug, Clone)]
pub enum Source {
    Loose(PathBuf),
    Vol { archive: String, name: String },
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Loose(path) => write!(f, "{}", path.display()),
            Source::Vol { archive, name } => write!(f, "{archive}:{name}"),
        }
    }
}

pub struct GameFiles {
    root: PathBuf,
    /// The folders indexed for loose files, in priority order.
    dirs: Vec<PathBuf>,
    /// Lowercase file name -> first loose file found, in priority order: its folder in `dirs` and its path there
    /// (relative, so the index doesn't hold the folder's path once per file).
    loose: HashMap<String, (usize, PathBuf)>,
    vols: Vec<(String, VolArchive)>,
}

impl GameFiles {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        let mut dirs = Vec::new();
        if let Some(opu) = crate::install::opu_dir(&root) {
            // Any case: an install copied from Windows may have `BASE` or `Maps` on a case-sensitive filesystem.
            dirs.extend(["base", "maps"].into_iter().filter_map(|dir| crate::install::resolve_ci(&opu, Path::new(dir))));
        }
        let mut loose = HashMap::new();
        for (k, d) in dirs.iter().enumerate() {
            index_dir(d, d, k, true, &mut loose);
        }
        dirs.push(root.clone());
        index_dir(&root, &root, dirs.len() - 1, false, &mut loose);

        let mut vols = Vec::new();
        let vol_paths: Vec<_> = read_dir_sorted(&root)
            .into_iter()
            .map(|(p, _)| p)
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vol")))
            .collect();
        for path in vol_paths {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            vols.push((name, VolArchive::open(&path)?));
        }

        Ok(Self { root, dirs, loose, vols })
    }

    /// Opens `dir`, else [`default_game_dir`](crate::default_game_dir); an error names the folder.
    pub fn open_game_dir(dir: Option<PathBuf>) -> Result<Self, Error> {
        let dir = dir.unwrap_or_else(crate::default_game_dir);
        Self::open(&dir).map_err(|e| Error::Invalid(format!("opening game folder {}: {e}", dir.display())))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads and parses a map (a name like `cm01.map`, or a path); a parse error names where the map came from.
    pub fn read_map(&self, name: &str) -> Result<crate::Map, Error> {
        let (data, source) = self.read(name)?;
        crate::Map::parse(&data).map_err(|e| Error::Invalid(format!("parsing {source}: {e}")))
    }

    /// Reads `name` (a bare file name like `cm01.map`), or a path if it exists on disk.
    pub fn read(&self, name: &str) -> Result<(Vec<u8>, Source), Error> {
        let as_path = Path::new(name);
        if as_path.components().count() > 1 && as_path.is_file() {
            let data = fs::read(as_path).map_err(|source| io_err(as_path, source))?;
            return Ok((data, Source::Loose(as_path.to_path_buf())));
        }
        if let Some(path) = self.loose_path(name) {
            let data = fs::read(&path).map_err(|source| io_err(&path, source))?;
            return Ok((data, Source::Loose(path)));
        }
        for (archive, vol) in &self.vols {
            if let Some(entry) = vol.find(name) {
                let source = Source::Vol { archive: archive.clone(), name: entry.name.clone() };
                return Ok((vol.data(entry)?.into_owned(), source));
            }
        }
        Err(Error::NotFound(name.to_string()))
    }

    /// The loose file `name` resolves to, if any (for files too big to read whole, like `op2.clm`).
    pub fn loose_path(&self, name: &str) -> Option<PathBuf> {
        self.loose.get(&name.to_ascii_lowercase()).map(|(dir, rel)| self.dirs[*dir].join(rel))
    }

    /// Every distinct `.map` file name available, sorted.
    pub fn map_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .loose
            .values()
            .filter_map(|(_, p)| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .chain(self.vols.iter().flat_map(|(_, v)| v.entries().iter().map(|e| e.name.clone())))
            .filter(|n| n.to_ascii_lowercase().ends_with(".map"))
            .collect();
        names.sort_by_key(|n| n.to_ascii_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names
    }

    pub fn vols(&self) -> impl Iterator<Item = (&str, &VolArchive)> {
        self.vols.iter().map(|(n, v)| (n.as_str(), v))
    }
}

/// The file `name` in `dir` itself (not OPU's overrides), matched case-insensitively as Windows would: installs mix
/// cases (`OP2Shell.dll`, `op2shres.dll`, `e01.dll`) and a case-sensitive filesystem keeps them.
pub fn find_in_dir(dir: &Path, name: &str) -> Result<PathBuf, Error> {
    let entries = fs::read_dir(dir).map_err(|source| io_err(dir, source))?;
    entries
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
        .ok_or_else(|| Error::Invalid(format!("{name} not found in {}", dir.display())))
}

/// Indexes the files in `dir` (in `top`, folder `k` of [`GameFiles::dirs`]) by lowercase name, keeping the first.
fn index_dir(top: &Path, dir: &Path, k: usize, recursive: bool, out: &mut HashMap<String, (usize, PathBuf)>) {
    for (path, is_dir) in read_dir_sorted(dir) {
        if is_dir {
            if recursive {
                index_dir(top, &path, k, true, out);
            }
        } else if let (Some(name), Ok(rel)) = (path.file_name(), path.strip_prefix(top)) {
            out.entry(name.to_string_lossy().to_ascii_lowercase()).or_insert_with(|| (k, rel.to_path_buf()));
        }
    }
}

/// A folder's entries, sorted, each with whether it is a folder. The listing gives the type on most systems (no stat
/// per file: indexing a game folder is part of every mission start); a link is followed.
fn read_dir_sorted(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut paths: Vec<(PathBuf, bool)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let path = e.path();
            let is_dir = match e.file_type() {
                Ok(t) if !t.is_symlink() => t.is_dir(),
                _ => path.is_dir(),
            };
            (path, is_dir)
        })
        .collect();
    paths.sort();
    paths
}

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.display().to_string(), source }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OPU's `base` and `maps` are found in any case (an install copied from Windows onto a case-sensitive
    /// filesystem), and `base` still overrides `maps`, which overrides the game folder.
    #[test]
    fn opu_folders_in_any_case() {
        let root = std::env::temp_dir().join(format!("op2-game-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("OPU/BASE/Sub")).unwrap();
        fs::create_dir_all(root.join("OPU/Maps")).unwrap();
        fs::write(root.join("OPU/BASE/Sub/Language.INI"), b"base").unwrap();
        fs::write(root.join("OPU/Maps/language.ini"), b"maps").unwrap();
        fs::write(root.join("OPU/Maps/X.MAP"), b"map").unwrap();
        fs::write(root.join("x.map"), b"game").unwrap();
        let files = GameFiles::open(&root).unwrap();
        let read = |n: &str| files.read(n).map(|(d, _)| d);
        assert_eq!(read("language.ini").unwrap(), b"base");
        assert_eq!(read("x.map").unwrap(), b"map");
        let _ = fs::remove_dir_all(&root);
    }

    /// A linked folder is indexed as a folder and a linked file as a file (the listing's own type is the link's).
    #[cfg(unix)]
    #[test]
    fn links_are_followed() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("op2-game-files-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(elsewhere.join("maps")).unwrap();
        fs::write(elsewhere.join("maps/linked.map"), b"linked").unwrap();
        fs::write(elsewhere.join("real.vol"), b"not a vol").unwrap();
        let game = root.join("game");
        fs::create_dir_all(game.join("OPU")).unwrap();
        symlink(elsewhere.join("maps"), game.join("OPU/maps")).unwrap();
        symlink(elsewhere.join("real.vol"), game.join("sheets.vol")).unwrap();
        assert_eq!(read_dir_sorted(&game), vec![(game.join("OPU"), true), (game.join("sheets.vol"), false)]);
        assert_eq!(read_dir_sorted(&game.join("OPU")), vec![(game.join("OPU/maps"), true)]);
        let maps = game.join("OPU/maps");
        let mut index = HashMap::new();
        index_dir(&maps, &maps, 0, true, &mut index);
        assert_eq!(index.get("linked.map"), Some(&(0, PathBuf::from("linked.map"))));
        // GameFiles::open takes the linked .vol as an archive (and fails on its bytes, as on any bad volume).
        assert!(GameFiles::open(&game).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
