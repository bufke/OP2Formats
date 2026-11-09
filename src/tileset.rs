//! Tileset images: a vertical strip of 32x32 tiles in 8-bit indexed color. Two encodings exist:
//! standard Windows BMP (what OPU ships loose), and the game's own "PBMP" container, which
//! stores the palette with red and blue swapped and pixels top-down.

use crate::Error;
use crate::reader::Reader;

pub const TILE_SIZE: u32 = 32;

/// Decoded tileset, `TILE_SIZE` wide and `tile_count * TILE_SIZE` tall: palette indices and the palette, a quarter
/// of the memory RGBA would take (e01's tilesets: ~2 MiB instead of ~8). [`TilePixels`] gives RGBA.
#[derive(Debug, Clone)]
pub struct Tileset {
    pub tile_count: u32,
    indices: Vec<u8>,
    palette: Box<[[u8; 4]; 256]>,
}

/// Bytes of one tile as RGBA.
pub const TILE_RGBA_BYTES: usize = (TILE_SIZE * TILE_SIZE * 4) as usize;

/// One tile of a [`Tileset`]: its row-major palette indices and the palette.
#[derive(Debug, Clone, Copy)]
pub struct TilePixels<'a> {
    indices: &'a [u8],
    palette: &'a [[u8; 4]; 256],
}

impl TilePixels<'_> {
    /// The RGBA colour (alpha 255) at (x, y) in the tile.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.palette[self.indices[(y * TILE_SIZE + x) as usize] as usize]
    }

    /// Row `y` as RGBA into `dst` (`TILE_SIZE * 4` bytes).
    pub fn write_row(&self, y: u32, dst: &mut [u8]) {
        let row = &self.indices[(y * TILE_SIZE) as usize..][..TILE_SIZE as usize];
        for (d, &i) in dst.as_chunks_mut::<4>().0.iter_mut().zip(row) {
            *d = self.palette[i as usize];
        }
    }

    /// Appends the tile's RGBA pixels, row-major, [`TILE_RGBA_BYTES`] bytes.
    pub fn extend_rgba(&self, out: &mut Vec<u8>) {
        out.extend(self.indices.iter().flat_map(|&i| self.palette[i as usize]));
    }

    /// The tile's RGBA pixels, row-major, [`TILE_RGBA_BYTES`] bytes.
    pub fn rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(TILE_RGBA_BYTES);
        self.extend_rgba(&mut v);
        v
    }
}

impl Tileset {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        match data.get(..4) {
            Some(b"PBMP") => parse_pbmp(data),
            Some([b'B', b'M', ..]) => parse_bmp(data),
            _ => Err(Error::Invalid("tileset is neither a BMP nor a PBMP file".into())),
        }
    }

    /// One tile's pixels.
    pub fn tile(&self, index: u32) -> Option<TilePixels<'_>> {
        let len = (TILE_SIZE * TILE_SIZE) as usize;
        let start = index as usize * len;
        Some(TilePixels { indices: self.indices.get(start..start + len)?, palette: &self.palette })
    }

    /// The whole strip as RGBA, row-major.
    pub fn rgba(&self) -> Vec<u8> {
        self.indices.iter().flat_map(|&i| self.palette[i as usize]).collect()
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

fn decode<'a>(height: u32, palette: &[[u8; 3]; 256], row: impl Fn(u32) -> Result<&'a [u8], Error>) -> Result<Tileset, Error> {
    let mut indices = Vec::with_capacity((TILE_SIZE * height) as usize);
    for y in 0..height {
        indices.extend_from_slice(&row(y)?[..TILE_SIZE as usize]);
    }
    Ok(Tileset { tile_count: height / TILE_SIZE, indices, palette: Box::new(palette.map(|[r, g, b]| [r, g, b, 255])) })
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
    decode(height, &palette, |y| {
        let src_row = if bottom_up { height - 1 - y } else { y } as usize;
        Reader::at(data, pixel_offset + src_row * stride).bytes(stride)
    })
}

/// A PBMP tileset as the plain BMP OPU's installer writes for it (`OPU/base/tilesets`): an 8-bit top-down DIB with
/// the palette's red and blue swapped into BMP order (the fourth byte kept) and the pixels unchanged.
pub fn pbmp_to_bmp(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut r = Reader::new(data);
    r.expect_tag(b"PBMP")?;
    r.u32()?;
    r.expect_tag(b"head")?;
    r.u32()?;
    r.u32()?;
    let width = r.u32()?;
    let height = r.u32()?;
    let bit_depth = r.u32()?;
    r.u32()?;
    if bit_depth != 8 {
        return Err(Error::Unsupported(format!("PBMP with {bit_depth} bits per pixel")));
    }
    r.expect_tag(b"PPAL")?;
    r.u32()?;
    r.expect_tag(b"head")?;
    r.u32()?;
    r.u32()?;
    r.expect_tag(b"data")?;
    r.u32()?;
    let mut palette = Vec::with_capacity(1024);
    for _ in 0..256 {
        let rgbx = r.bytes(4)?;
        palette.extend_from_slice(&[rgbx[2], rgbx[1], rgbx[0], rgbx[3]]);
    }
    r.expect_tag(b"data")?;
    r.u32()?;
    let stride = (width as usize).div_ceil(4) * 4;
    let pixels = r.bytes(stride * height as usize)?;
    let offset = 14 + 40 + palette.len();
    let mut out = Vec::with_capacity(offset + pixels.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((offset + pixels.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&(-(height as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    out.extend_from_slice(&palette);
    out.extend_from_slice(pixels);
    Ok(out)
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
    decode(height, &palette, |y| Reader::at(data, pixel_start + y as usize * stride).bytes(stride))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-tile PBMP with a gradient and a palette whose entries all differ.
    fn pbmp() -> Vec<u8> {
        let mut d = Vec::new();
        let tag = |d: &mut Vec<u8>, t: &[u8; 4], v: u32| {
            d.extend(t);
            d.extend(v.to_le_bytes());
        };
        tag(&mut d, b"PBMP", 0);
        tag(&mut d, b"head", 20);
        for v in [2, 32, 32, 8, 0] {
            d.extend(u32::to_le_bytes(v));
        }
        tag(&mut d, b"PPAL", 0);
        tag(&mut d, b"head", 4);
        d.extend(1u32.to_le_bytes());
        tag(&mut d, b"data", 1024);
        for i in 0..256u32 {
            d.extend([i as u8, (i * 3) as u8, (255 - i) as u8, 4]);
        }
        tag(&mut d, b"data", 1024);
        d.extend((0..1024u32).map(|i| (i * 7 / 5) as u8));
        d
    }

    #[test]
    fn pbmp_converts_to_the_same_picture_as_a_bmp() {
        let p = pbmp();
        let bmp = pbmp_to_bmp(&p).unwrap();
        assert_eq!(bmp.len(), 14 + 40 + 1024 + 1024);
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), -32, "top-down");
        assert_eq!(&bmp[54 + 4..54 + 8], &[255 - 1, 3, 1, 4], "palette entry 1 in BGR order, fourth byte kept");
        assert_eq!(Tileset::parse(&bmp).unwrap().rgba(), Tileset::parse(&p).unwrap().rgba());
        assert!(pbmp_to_bmp(&p[..p.len() - 1]).is_err());
    }
}
