//! Cinepak (`cvid`) video decoder, as used by the game's 640x480 cutscenes.
//!
//! A frame is split into horizontal strips. Each strip carries (optionally updated) codebooks of
//! 2x2-pixel YUV vectors and then one entry per 4x4 block: a "V1" block is one vector scaled up
//! 2x, a "V4" block is four vectors. Inter frames may skip blocks, keeping the previous frame's
//! pixels. Codebooks persist per strip index across frames, and a strip without codebook updates
//! starts from the previous strip's books (unless frame flag bit 0 is set).
//!
//! Colour conversion follows the reference decoder (integer, chroma halved for green), so output
//! is bit-exact with libavcodec's RGB24 output.

use crate::Error;

const MAX_STRIPS: usize = 32;

type Entry = [[u8; 3]; 4]; // 2x2 pixels as RGB: top-left, top-right, bottom-left, bottom-right
type Codebook = [Entry; 256];

pub struct Cinepak {
    pub width: u32,
    pub height: u32,
    /// The current picture as RGB24, top row first.
    pub rgb: Vec<u8>,
    v1: Vec<Codebook>,
    v4: Vec<Codebook>,
}

fn be16(d: &[u8], i: usize) -> Result<usize, Error> {
    d.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize).ok_or(Error::Truncated { offset: i, wanted: 2 })
}

fn be24(d: &[u8], i: usize) -> Result<usize, Error> {
    d.get(i..i + 3)
        .map(|b| ((b[0] as usize) << 16) | ((b[1] as usize) << 8) | b[2] as usize)
        .ok_or(Error::Truncated { offset: i, wanted: 3 })
}

impl Cinepak {
    pub fn new(width: u32, height: u32) -> Self {
        let books = || vec![[[[0u8; 3]; 4]; 256]; MAX_STRIPS];
        Self { width, height, rgb: vec![0; (width * height * 3) as usize], v1: books(), v4: books() }
    }

    /// Decodes one frame on top of the previous picture. An empty frame repeats the picture.
    pub fn decode(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.is_empty() {
            return Ok(());
        }
        if data.len() < 10 {
            return Err(Error::Truncated { offset: 0, wanted: 10 });
        }
        let flags = data[0];
        let strips = be16(data, 8)?;
        if strips > MAX_STRIPS {
            return Err(Error::Invalid(format!("cinepak frame with {strips} strips")));
        }
        let mut pos = 10;
        let mut y0 = 0usize;
        for i in 0..strips {
            if pos + 12 > data.len() {
                return Err(Error::Truncated { offset: pos, wanted: 12 });
            }
            let id = data[pos];
            if id != 0x10 && id != 0x11 {
                return Err(Error::Invalid(format!("cinepak strip id {id:#x} at {pos:#x}")));
            }
            let size = be24(data, pos + 1)?;
            // The y2 field is the strip's height; strips stack top to bottom.
            let strip_h = be16(data, pos + 8)?;
            if size < 12 {
                return Err(Error::Invalid(format!("cinepak strip size {size}")));
            }
            let end = (pos + size).min(data.len());
            if i > 0 && flags & 1 == 0 {
                self.v1[i] = self.v1[i - 1];
                self.v4[i] = self.v4[i - 1];
            }
            let (y1, y2) = (y0, (y0 + strip_h).min(self.height as usize));
            self.decode_strip(i, &data[pos + 12..end], y1, y2)?;
            y0 = y2;
            pos = end;
        }
        Ok(())
    }

    fn decode_strip(&mut self, strip: usize, data: &[u8], y1: usize, y2: usize) -> Result<(), Error> {
        let mut pos = 0;
        while pos + 4 <= data.len() {
            let id = data[pos];
            let size = be24(data, pos + 1)?.checked_sub(4).ok_or_else(|| Error::Invalid("cinepak chunk size".into()))?;
            pos += 4;
            let end = (pos + size).min(data.len());
            let chunk = &data[pos..end];
            match id {
                0x20 | 0x21 | 0x24 | 0x25 => decode_codebook(&mut self.v4[strip], id, chunk),
                0x22 | 0x23 | 0x26 | 0x27 => decode_codebook(&mut self.v1[strip], id, chunk),
                0x30..=0x32 => return self.decode_vectors(strip, id, chunk, y1, y2),
                _ => {}
            }
            pos = end;
        }
        Ok(())
    }

    fn decode_vectors(&mut self, strip: usize, id: u8, data: &[u8], y1: usize, y2: usize) -> Result<(), Error> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut pos = 0;
        let mut flag = 0u32;
        let mut mask = 0u32;
        let truncated = |pos| Error::Truncated { offset: pos, wanted: 4 };
        let next_flag = |pos: &mut usize, flag: &mut u32, mask: &mut u32| -> Result<(), Error> {
            *mask >>= 1;
            if *mask == 0 {
                let b = data.get(*pos..*pos + 4).ok_or_else(|| truncated(*pos))?;
                *flag = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                *pos += 4;
                *mask = 0x8000_0000;
            }
            Ok(())
        };
        for by in (y1..y2).step_by(4) {
            for bx in (0..w).step_by(4) {
                if id & 1 != 0 {
                    next_flag(&mut pos, &mut flag, &mut mask)?;
                    if flag & mask == 0 {
                        continue; // skipped: keep the previous frame's block
                    }
                }
                let v4 = if id & 2 != 0 {
                    false
                } else {
                    next_flag(&mut pos, &mut flag, &mut mask)?;
                    flag & mask != 0
                };
                if v4 {
                    let idx = data.get(pos..pos + 4).ok_or_else(|| truncated(pos))?;
                    pos += 4;
                    for (q, &i) in idx.iter().enumerate() {
                        let e = &self.v4[strip][i as usize];
                        let (qx, qy) = (bx + (q & 1) * 2, by + (q >> 1) * 2);
                        for (k, px) in e.iter().enumerate() {
                            put(&mut self.rgb, w, h, qx + (k & 1), qy + (k >> 1), *px);
                        }
                    }
                } else {
                    let i = *data.get(pos).ok_or_else(|| truncated(pos))?;
                    pos += 1;
                    let e = &self.v1[strip][i as usize];
                    for (k, px) in e.iter().enumerate() {
                        let (ex, ey) = (bx + (k & 1) * 2, by + (k >> 1) * 2);
                        for d in 0..4 {
                            put(&mut self.rgb, w, h, ex + (d & 1), ey + (d >> 1), *px);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Whether a frame decodes without anything from the frames before it, so playback can seek to it: every strip codes
/// all its blocks (vector chunk 0x30 or 0x32, no 0x31 skips), the strips cover all `height` rows, and every strip's
/// codebooks are fully defined in this frame (full codebook chunks, or the strip above's books when frame flag bit 0
/// is clear, plus any selective updates on top). The strip id doesn't decide it: the game's keyframes are an intra
/// (0x10) strip with full books followed by 0x11 strips that code every block with selectively updated books. An
/// empty frame (a repeat) is never one.
pub fn is_keyframe(data: &[u8], height: u32) -> bool {
    let (Ok(strips), Some(&flags)) = (be16(data, 8), data.first()) else { return false };
    let (mut pos, mut rows) = (10, 0);
    let (mut v4, mut v1) = (false, false);
    for i in 0..strips.min(MAX_STRIPS) {
        let (Some(&id), Ok(size), Ok(strip_h)) = (data.get(pos), be24(data, pos + 1), be16(data, pos + 8)) else { return false };
        if !matches!(id, 0x10 | 0x11) || size < 12 {
            return false;
        }
        if i == 0 || flags & 1 != 0 {
            (v4, v1) = (false, false); // Starts from this strip index's books of the previous frame.
        }
        let end = (pos + size).min(data.len());
        let mut c = pos + 12;
        while c + 4 <= end {
            let Ok(len) = be24(data, c + 1) else { return false };
            match data[c] {
                0x20 | 0x24 => v4 = true,
                0x22 | 0x26 => v1 = true,
                0x30 | 0x32 => {}
                0x31 => return false, // inter-coded vectors: some blocks keep the previous picture
                _ => {}
            }
            c += len.max(4);
        }
        if !(v4 && v1) {
            return false;
        }
        rows += strip_h;
        pos = end;
    }
    strips > 0 && rows >= height as usize
}

fn put(rgb: &mut [u8], w: usize, h: usize, x: usize, y: usize, px: [u8; 3]) {
    if x < w && y < h {
        let o = (y * w + x) * 3;
        rgb[o..o + 3].copy_from_slice(&px);
    }
}

/// Codebook chunk: bit 2 of the id = greyscale (4-byte entries, no chroma), bit 0 = selective
/// update (a 32-bit big-endian mask before each run of 32 entries).
fn decode_codebook(book: &mut Codebook, id: u8, data: &[u8]) {
    let n = if id & 4 != 0 { 4 } else { 6 };
    let mut pos = 0;
    let mut flag = 0u32;
    let mut mask = 0u32;
    for entry in book.iter_mut() {
        if id & 1 != 0 {
            mask >>= 1;
            if mask == 0 {
                let Some(b) = data.get(pos..pos + 4) else { return };
                flag = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                pos += 4;
                mask = 0x8000_0000;
            }
            if flag & mask == 0 {
                continue;
            }
        }
        let Some(v) = data.get(pos..pos + n) else { return };
        pos += n;
        let (u, vv) = if n == 6 { (v[4] as i8 as i32, v[5] as i8 as i32) } else { (0, 0) };
        for k in 0..4 {
            let y = v[k] as i32;
            entry[k] = if n == 6 {
                [clamp(y + vv * 2), clamp(y - u / 2 - vv), clamp(y + u * 2)]
            } else {
                [v[k], v[k], v[k]]
            };
        }
    }
}

fn clamp(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One 8x4 intra strip: a V1 codebook entry, a V4 codebook entry, one V1 block, one V4 block.
    #[test]
    fn intra_strip() {
        let mut chunks = Vec::new();
        let chunk = |id: u8, body: &[u8]| {
            let n = body.len() + 4;
            let mut c = vec![id, (n >> 16) as u8, (n >> 8) as u8, n as u8];
            c.extend_from_slice(body);
            c
        };
        chunks.extend(chunk(0x22, &[10, 20, 30, 40, 0, 0])); // V1 entry 0, grey ramp
        chunks.extend(chunk(0x20, &[100, 100, 100, 100, 10, 0])); // V4 entry 0: u = 10
        // Vectors: flag word, bit 31 clear = block 0 is V1, bit 30 set = block 1 is V4.
        chunks.extend(chunk(0x30, &[0x40, 0, 0, 0, 0, 0, 0, 0, 0]));
        let strip_len = 12 + chunks.len();
        let mut strip = vec![0x10, (strip_len >> 16) as u8, (strip_len >> 8) as u8, strip_len as u8];
        strip.extend_from_slice(&[0, 0, 0, 0, 0, 4, 0, 8]); // y1, x1, height 4, x2
        strip.extend(chunks);
        let len = 10 + strip.len();
        let mut frame = vec![0, (len >> 16) as u8, (len >> 8) as u8, len as u8, 0, 8, 0, 4, 0, 1];
        frame.extend(strip);

        assert!(is_keyframe(&frame, 4));
        assert!(!is_keyframe(&frame, 8), "strips must cover the picture");
        let mut partial = frame.clone();
        partial[10 + 12] = 0x23; // the V1 book only updated selectively: depends on the previous frame
        assert!(!is_keyframe(&partial, 4));
        assert!(!is_keyframe(&[], 4));

        let mut dec = Cinepak::new(8, 4);
        dec.decode(&frame).unwrap();
        let px = |x: usize, y: usize| &dec.rgb[(y * 8 + x) * 3..][..3];
        assert_eq!(px(0, 0), &[10, 10, 10]);
        assert_eq!(px(1, 1), &[10, 10, 10]);
        assert_eq!(px(2, 0), &[20, 20, 20]);
        assert_eq!(px(3, 3), &[40, 40, 40]);
        // u = 10: r = y, g = y - 5, b = y + 20.
        assert_eq!(px(4, 0), &[100, 95, 120]);
        assert_eq!(px(7, 3), &[100, 95, 120]);
    }
}
