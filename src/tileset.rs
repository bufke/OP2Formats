//! Tileset images: a vertical strip of 32x32 tiles in 8-bit indexed color. Two encodings exist:
//! standard Windows BMP (what OPU ships loose), and the game's own "PBMP" container, which
//! stores the palette with red and blue swapped and pixels top-down.

use crate::Error;
use crate::reader::Reader;

pub const TILE_SIZE: u32 = 32;

/// Decoded tileset, as RGBA8 pixels, `TILE_SIZE` wide and `tile_count * TILE_SIZE` tall.
#[derive(Debug, Clone)]
pub struct Tileset {
    pub tile_count: u32,
    pub rgba: Vec<u8>,
}

impl Tileset {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        match data.get(..4) {
            Some(b"PBMP") => parse_pbmp(data),
            Some([b'B', b'M', ..]) => parse_bmp(data),
            _ => Err(Error::Invalid("tileset is neither a BMP nor a PBMP file".into())),
        }
    }

    /// RGBA pixels of one tile, row-major, `TILE_SIZE * TILE_SIZE * 4` bytes.
    pub fn tile_rgba(&self, index: u32) -> Option<&[u8]> {
        let len = (TILE_SIZE * TILE_SIZE * 4) as usize;
        let start = index as usize * len;
        self.rgba.get(start..start + len)
    }
}

fn check_dims(width: i64, height: i64, bit_depth: u32) -> Result<u32, Error> {
    if width != TILE_SIZE as i64 || bit_depth != 8 || height <= 0 || height % TILE_SIZE as i64 != 0 {
        return Err(Error::Invalid(format!(
            "tileset must be 32px wide, 8-bit, height a multiple of 32; got {width}x{height} at {bit_depth} bpp"
        )));
    }
    Ok(height as u32)
}

fn to_rgba<'a>(height: u32, palette: &[[u8; 3]; 256], row: impl Fn(u32) -> Result<&'a [u8], Error>) -> Result<Tileset, Error> {
    let mut rgba = Vec::with_capacity((TILE_SIZE * height * 4) as usize);
    for y in 0..height {
        for &index in &row(y)?[..TILE_SIZE as usize] {
            let [r, g, b] = palette[index as usize];
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Ok(Tileset { tile_count: height / TILE_SIZE, rgba })
}

fn parse_bmp(data: &[u8]) -> Result<Tileset, Error> {
    let mut r = Reader::at(data, 10);
    let pixel_offset = r.u32()? as usize;
    let info_size = r.u32()? as usize;
    let width = r.i32()? as i64;
    let raw_height = r.i32()? as i64;
    r.u16()?; // planes
    let bit_depth = r.u16()? as u32;
    let compression = r.u32()?;
    if compression != 0 {
        return Err(Error::Unsupported(format!("compressed BMP tileset (compression {compression})")));
    }
    r.skip(12)?; // image size, x/y resolution
    let colors_used = match r.u32()? {
        0 => 256,
        n => n.min(256) as usize,
    };
    let height = check_dims(width, raw_height.abs(), bit_depth)?;

    let mut palette = [[0u8; 3]; 256];
    let mut pr = Reader::at(data, 14 + info_size);
    for entry in palette.iter_mut().take(colors_used) {
        let bgrx = pr.bytes(4)?;
        *entry = [bgrx[2], bgrx[1], bgrx[0]];
    }

    // Rows are padded to 4 bytes; 32px at 8bpp needs none. Positive height means bottom-up.
    let stride = TILE_SIZE as usize;
    let bottom_up = raw_height > 0;
    to_rgba(height, &palette, |y| {
        let src_row = if bottom_up { height - 1 - y } else { y } as usize;
        Reader::at(data, pixel_offset + src_row * stride).bytes(stride)
    })
}

fn parse_pbmp(data: &[u8]) -> Result<Tileset, Error> {
    let mut r = Reader::new(data);
    r.expect_tag(b"PBMP")?;
    r.u32()?;
    r.expect_tag(b"head")?;
    r.u32()?;
    r.u32()?; // tag count
    let width = r.u32()? as i64;
    let height = r.u32()? as i64;
    let bit_depth = r.u32()?;
    r.u32()?; // flags
    let height = check_dims(width, height, bit_depth)?;

    r.expect_tag(b"PPAL")?;
    r.u32()?;
    r.expect_tag(b"head")?;
    r.u32()?;
    r.u32()?;
    r.expect_tag(b"data")?;
    r.u32()?;
    let mut palette = [[0u8; 3]; 256];
    for entry in palette.iter_mut() {
        let rgbx = r.bytes(4)?;
        *entry = [rgbx[0], rgbx[1], rgbx[2]];
    }

    r.expect_tag(b"data")?;
    r.u32()?;
    let pixel_start = r.pos();
    let stride = TILE_SIZE as usize;
    to_rgba(height, &palette, |y| Reader::at(data, pixel_start + y as usize * stride).bytes(stride))
}
