//! Readers for Outpost 2 data files. Formats follow OutpostUniverse/OP2Utility.

pub mod adpcm;
pub mod art;
pub mod avi;
pub mod cinepak;
mod game_files;
pub mod lzh;
pub mod map;
mod reader;
pub mod pe_res;
pub mod render;
pub mod sheets;
pub mod sound;
pub mod strings;
#[doc(hidden)]
pub mod testdata;
pub mod tile_palette;
pub mod tileset;
pub mod units;
pub mod vol;

pub use art::{ArtFile, Sprite, Sprites, Tint};
pub use game_files::{GameFiles, Source};
pub use map::{CellType, Map, Speed, Tile};
pub use tileset::{TILE_SIZE, Tileset};
pub use vol::VolArchive;

/// Game folder to use: `$OP2_DIR`, else the Wine prefix this workspace sits next to.
pub fn default_game_dir() -> std::path::PathBuf {
    std::env::var_os("OP2_DIR").map(Into::into).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../prefix/drive_c/GOG Games/Outpost 2")
    })
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
