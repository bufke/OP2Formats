//! Readers for Outpost 2 data files. Formats follow OutpostUniverse/OP2Utility.

pub mod adpcm;
pub mod art;
pub mod avi;
pub mod cinepak;
pub mod exe;
mod game_files;
#[cfg(feature = "inno")]
pub mod inno;
pub mod install;
pub mod lzh;
pub mod map;
#[cfg(feature = "inno")]
mod md5;
#[cfg(feature = "opu")]
pub mod nsis;
pub mod opu;
mod reader;
pub mod pe_res;
pub mod pick;
pub mod render;
#[cfg(feature = "opu")]
mod sha256;
pub mod sheets;
pub mod sound;
pub mod strings;
#[doc(hidden)]
pub mod testdata;
pub mod tile_palette;
pub mod tileset;
pub mod units;
pub mod virmask;
pub mod vol;

pub use art::{ArtFile, Sprite, Sprites, Tint};
pub use game_files::{GameFiles, Source, find_in_dir};
pub use map::{CellType, Map, Speed, Tile};
pub use tileset::{TILE_RGBA_BYTES, TILE_SIZE, TilePixels, Tileset};
pub use vol::VolArchive;

static GAME_DIR: std::sync::RwLock<Option<std::path::PathBuf>> = std::sync::RwLock::new(None);
static FOUND_GAME_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Sets the game folder [`default_game_dir`] returns for the rest of the process. The shipped programs call this
/// once at startup with the folder [`install::resolve`] picked (command line, settings, discovery).
pub fn set_game_dir(dir: std::path::PathBuf) {
    *GAME_DIR.write().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

/// Game folder to use: the one given to [`set_game_dir`], else `$OP2_DIR`, else this repository's Wine prefix when
/// the program runs from the source tree, else the first complete install in [`install::known_locations`]. With
/// none of those, a path that doesn't exist (so opening it fails with a message naming it).
pub fn default_game_dir() -> std::path::PathBuf {
    if let Some(d) = GAME_DIR.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return d;
    }
    if let Some(d) = std::env::var_os("OP2_DIR").filter(|v| !v.is_empty()) {
        return d.into();
    }
    FOUND_GAME_DIR
        .get_or_init(|| {
            source_tree_game_dir()
                .or_else(|| install::search(&install::Env::current()).best().map(|(c, _)| c.dir.clone()))
                .unwrap_or_else(|| std::path::PathBuf::from("<no Outpost 2 install found>"))
        })
        .clone()
}

/// This repository's Wine prefix install (`prefix/drive_c/GOG Games/Outpost 2`), when the running program was built
/// in and runs from this source tree (`tools/target/...`: tests, the harness, `cargo run`) and the folder exists.
/// A release binary run anywhere else never looks there.
pub fn source_tree_game_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let tools = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = tools.join("target").canonicalize().ok()?;
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    if !exe.starts_with(&target) {
        return None;
    }
    let dir = tools.join("../prefix/drive_c/GOG Games/Outpost 2").canonicalize().ok().filter(|d| d.is_dir())?;
    // Windows's canonical form is a `\\?\` path; the plain one reads better in messages and works the same here.
    let plain = dir.to_str().and_then(|s| s.strip_prefix(r"\\?\")).filter(|s| !s.starts_with("UNC")).map(PathBuf::from);
    Some(plain.unwrap_or(dir))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unexpected end of data at offset {offset:#x} (wanted {wanted} bytes)")]
    Truncated { offset: usize, wanted: usize },
    #[error("expected tag {:?} at offset {offset:#x}, found {:?}", String::from_utf8_lossy(expected), String::from_utf8_lossy(found))]
    BadTag { offset: usize, expected: [u8; 4], found: [u8; 4] },
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0} not found in loose files or .vol archives")]
    NotFound(String),
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
}
