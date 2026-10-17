//! Finds game files by name the way an OPU install layers them: loose files under `OPU/base`
//! override `OPU/maps`, which override the game folder, which overrides `.vol` archives.
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
    /// Lowercase file name -> first loose file found, in priority order.
    loose: HashMap<String, PathBuf>,
    vols: Vec<(String, VolArchive)>,
}

impl GameFiles {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        let mut loose = HashMap::new();
        for dir in ["OPU/base", "OPU/maps"] {
            index_dir(&root.join(dir), true, &mut loose);
        }
        index_dir(&root, false, &mut loose);

        let mut vols = Vec::new();
        let mut vol_paths: Vec<_> = read_dir_sorted(&root)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vol")))
            .collect();
        vol_paths.sort();
        for path in vol_paths {
            let data = fs::read(&path).map_err(|source| io_err(&path, source))?;
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            vols.push((name, VolArchive::parse(data)?));
        }

        Ok(Self { root, loose, vols })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads `name` (a bare file name like `cm01.map`), or a path if it exists on disk.
    pub fn read(&self, name: &str) -> Result<(Vec<u8>, Source), Error> {
        let as_path = Path::new(name);
        if as_path.components().count() > 1 && as_path.is_file() {
            let data = fs::read(as_path).map_err(|source| io_err(as_path, source))?;
            return Ok((data, Source::Loose(as_path.to_path_buf())));
        }
        if let Some(path) = self.loose.get(&name.to_ascii_lowercase()) {
            let data = fs::read(path).map_err(|source| io_err(path, source))?;
            return Ok((data, Source::Loose(path.clone())));
        }
        for (archive, vol) in &self.vols {
            if let Some(entry) = vol.find(name) {
                let source = Source::Vol { archive: archive.clone(), name: entry.name.clone() };
                return Ok((vol.data(entry)?.into_owned(), source));
            }
        }
        Err(Error::NotFound(name.to_string()))
    }

    /// Every distinct `.map` file name available, sorted.
    pub fn map_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .loose
            .values()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
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

fn index_dir(dir: &Path, recursive: bool, out: &mut HashMap<String, PathBuf>) {
    for path in read_dir_sorted(dir) {
        if path.is_dir() {
            if recursive {
                index_dir(&path, true, out);
            }
        } else if let Some(name) = path.file_name() {
            out.entry(name.to_string_lossy().to_ascii_lowercase()).or_insert(path);
        }
    }
}

fn read_dir_sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    paths.sort();
    paths
}

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.display().to_string(), source }
}
