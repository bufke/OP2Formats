//! Sprites: `op2_art.prt` (palettes, image metadata, animations) plus `OP2_ART.BMP` (pixels).
//! Layout follows OP2Utility's ArtReader and OutpostUniverse/OP2ArtUtils' prt_file.py:
//!
//! ```text
//! "CPAL" u32 count     count x ("PPAL" 1048, "head" 4, u32 1, "data" 1024, 256 x [r, g, b, flags])
//! images               u32 count, count x 20 bytes (ImageMeta)
//! animation header     u32 animations, u32 frames, u32 layers, u32 optional_count
//! per animation        u32 unknown, rect (4 x i32), i32 x, i32 y, u32 unknown2, u32 frames,
//!                      frames..., u32 appendix_count, appendix_count x 16 bytes
//! per frame            u8 layers (bit 7: two i8 follow), u8 unknown (bit 7: two i8 follow),
//!                      layers x (u16 image, u8 unknown, u8 layer_index, i16 x, i16 y)
//! ```
//!
//! `OP2_ART.BMP` is a BMP file only in name: each image's pixels sit at
//! `bfOffBits + ImageMeta::offset`, top-down, either 8-bit indexed or (shadows) 1-bit.
//!
//! Palette bytes are stored R, G, B, flags, the reverse of a BMP's RGBQUAD. Palette indices
//! 0..24 of unit palettes are the player-color ramp; `color.bmp` holds one 24-entry ramp per
//! player (blue, red, green, yellow, cyan, magenta). Index 0 is transparent.

use crate::reader::Reader;
use crate::render::Image;
use crate::{Error, GameFiles};

pub const PLAYER_COLOR_RAMP: usize = 24;
pub const PLAYER_COLORS: usize = 6;
/// RGBA drawn for set pixels of 1-bit shadow images (the game darkens what is underneath).
pub const SHADOW_RGBA: [u8; 4] = [0, 0, 0, 112];

pub type Palette = [[u8; 3]; 256];

#[derive(Debug, Clone, Copy)]
pub struct ImageMeta {
    /// Bytes per row in `OP2_ART.BMP` as recorded; 1-bit images actually use `row_bytes()`.
    pub scanline_width: u32,
    pub offset: u32,
    pub height: u32,
    pub width: u32,
    /// Bit 0: in-game graphic (else menu). Bit 2: 1-bit shadow. Bit 6: truck bed (OP2Utility).
    pub kind: u16,
    pub palette: u16,
}

impl ImageMeta {
    pub fn is_shadow(&self) -> bool {
        self.kind & 0x4 != 0
    }

    /// Bytes per stored row. 8-bit rows are `width` rounded up to 4 bytes; 1-bit rows are
    /// `width` bits rounded up to 4 bytes (images are packed back to back that way).
    pub fn row_bytes(&self) -> usize {
        if self.is_shadow() { (self.width as usize).div_ceil(32) * 4 } else { self.scanline_width as usize }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub unknown: u32,
    pub selection: Rect,
    /// Frame-local position of the object's reference point (its pixel position on the map):
    /// the game draws a frame at `object_pixel - displacement`.
    pub displacement: (i32, i32),
    pub unknown2: u32,
    pub frames: Vec<Frame>,
    pub appendix: Vec<[u32; 4]>,
}

#[derive(Debug, Clone)]
pub struct Frame {
    /// Low 7 bits of the second frame byte; meaning unknown.
    pub unknown: u8,
    /// Optional offsets (TethysAPI `FrameOptionalInfo`). On vehicle chassis frames the first
    /// pair is where the turret attaches, relative to the reference point.
    pub optional1: Option<[i8; 2]>,
    pub optional2: Option<[i8; 2]>,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub image: u16,
    pub unknown: u8,
    pub index: u8,
    /// Top-left of the image in frame-local pixels.
    pub x: i16,
    pub y: i16,
}

/// Parsed `op2_art.prt`.
#[derive(Debug, Clone)]
pub struct ArtFile {
    pub palettes: Vec<Palette>,
    pub images: Vec<ImageMeta>,
    pub animations: Vec<Animation>,
    pub optional_count: u32,
}

impl ArtFile {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        r.expect_tag(b"CPAL")?;
        let palette_count = r.u32()?;
        let mut palettes = Vec::new();
        for _ in 0..palette_count {
            r.expect_tag(b"PPAL")?;
            r.u32()?;
            r.expect_tag(b"head")?;
            r.u32()?;
            r.u32()?;
            r.expect_tag(b"data")?;
            if r.u32()? != 1024 {
                return Err(Error::Invalid("palette data section is not 1024 bytes".into()));
            }
            let mut palette = [[0u8; 3]; 256];
            for entry in palette.iter_mut() {
                let rgbx = r.bytes(4)?;
                *entry = [rgbx[0], rgbx[1], rgbx[2]];
            }
            palettes.push(palette);
        }

        let image_count = r.u32()?;
        let mut images = Vec::new();
        for i in 0..image_count {
            let image = ImageMeta {
                scanline_width: r.u32()?,
                offset: r.u32()?,
                height: r.u32()?,
                width: r.u32()?,
                kind: r.u16()?,
                palette: r.u16()?,
            };
            if image.palette as usize >= palettes.len() {
                return Err(Error::Invalid(format!("image {i} uses palette {} of {}", image.palette, palettes.len())));
            }
            images.push(image);
        }

        let animation_count = r.u32()?;
        let frame_total = r.u32()?;
        let layer_total = r.u32()?;
        let optional_count = r.u32()?;
        let mut animations = Vec::new();
        let (mut frames_seen, mut layers_seen) = (0, 0);
        for _ in 0..animation_count {
            let unknown = r.u32()?;
            let selection = Rect { left: r.i32()?, top: r.i32()?, right: r.i32()?, bottom: r.i32()? };
            let displacement = (r.i32()?, r.i32()?);
            let unknown2 = r.u32()?;
            let frame_count = r.u32()?;
            let mut frames = Vec::new();
            for _ in 0..frame_count {
                let layer_byte = r.bytes(1)?[0];
                let unknown_byte = r.bytes(1)?[0];
                let mut pair = |flag: u8| -> Result<Option<[i8; 2]>, Error> {
                    Ok(if flag & 0x80 != 0 {
                        let b = r.bytes(2)?;
                        Some([b[0] as i8, b[1] as i8])
                    } else {
                        None
                    })
                };
                let optional1 = pair(layer_byte)?;
                let optional2 = pair(unknown_byte)?;
                let mut layers = Vec::new();
                for _ in 0..layer_byte & 0x7F {
                    let image = r.u16()?;
                    let flags = r.bytes(2)?;
                    let layer =
                        Layer { image, unknown: flags[0], index: flags[1], x: r.u16()? as i16, y: r.u16()? as i16 };
                    if layer.image as usize >= images.len() {
                        return Err(Error::Invalid(format!("layer references image {}", layer.image)));
                    }
                    layers.push(layer);
                }
                layers_seen += layers.len() as u32;
                frames.push(Frame { unknown: unknown_byte & 0x7F, optional1, optional2, layers });
            }
            frames_seen += frames.len() as u32;
            let appendix_count = r.u32()?;
            let mut appendix = Vec::new();
            for _ in 0..appendix_count {
                appendix.push([r.u32()?, r.u32()?, r.u32()?, r.u32()?]);
            }
            animations.push(Animation { unknown, selection, displacement, unknown2, frames, appendix });
        }
        if frames_seen != frame_total || layers_seen != layer_total {
            return Err(Error::Invalid(format!(
                "animation totals mismatch: header {frame_total} frames / {layer_total} layers, \
                 found {frames_seen} / {layers_seen}"
            )));
        }
        // Trailing bytes after the animations are unused by the game (OP2ArtUtils' extra.dat).
        Ok(Self { palettes, images, animations, optional_count })
    }
}

/// A composited frame. `hotspot` is the pixel in the image that sits on the object's map pixel
/// position, so draw the image with its top-left at `object_pixel - hotspot`.
#[derive(Debug, Clone)]
pub struct Sprite {
    pub image: Image,
    pub hotspot: (i32, i32),
}

/// Which colors to draw a sprite with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// The palette as stored (player-color slots hold player 0's blue).
    Stored,
    /// Player-color slots replaced with player `n`'s ramp from `color.bmp` (0..6).
    Player(u8),
}

/// Art metadata plus the `OP2_ART.BMP` pixel store, able to decode and composite frames.
pub struct Sprites {
    pub art: ArtFile,
    bmp: Vec<u8>,
    pixel_base: usize,
    /// `color.bmp` palette: `PLAYER_COLORS` ramps of `PLAYER_COLOR_RAMP` entries.
    player_ramps: Option<Vec<[u8; 3]>>,
}

impl Sprites {
    pub fn new(art: ArtFile, bmp: Vec<u8>, color_bmp: Option<&[u8]>) -> Result<Self, Error> {
        if bmp.get(..2) != Some(b"BM") {
            return Err(Error::Invalid("OP2_ART.BMP is not a BMP file".into()));
        }
        let pixel_base = Reader::at(&bmp, 10).u32()? as usize;
        let player_ramps = color_bmp.map(parse_color_bmp).transpose()?;
        Ok(Self { art, bmp, pixel_base, player_ramps })
    }

    /// Loads `op2_art.prt`, `OP2_ART.BMP` and (if present) `color.bmp` from the game files.
    pub fn load(files: &GameFiles) -> Result<Self, Error> {
        let (prt, _) = files.read("op2_art.prt")?;
        let (bmp, _) = files.read("OP2_ART.BMP")?;
        let color = files.read("color.bmp").ok().map(|(d, _)| d);
        Self::new(ArtFile::parse(&prt)?, bmp, color.as_deref())
    }

    pub fn animation(&self, index: usize) -> Option<&Animation> {
        self.art.animations.get(index)
    }

    /// Palette for `image` with the requested tint applied.
    pub fn palette(&self, image: &ImageMeta, tint: Tint) -> Palette {
        let mut palette = self.art.palettes[image.palette as usize];
        if let (Tint::Player(p), Some(ramps)) = (tint, &self.player_ramps)
            && image.palette != 0
        {
            let start = (p as usize).min(PLAYER_COLORS - 1) * PLAYER_COLOR_RAMP;
            palette[1..PLAYER_COLOR_RAMP].copy_from_slice(&ramps[start + 1..start + PLAYER_COLOR_RAMP]);
        }
        palette
    }

    /// Raw palette indices of an image, one byte per pixel (shadows: 0 or 1), row-major.
    pub fn image_indices(&self, index: usize) -> Result<Vec<u8>, Error> {
        let meta = self.art.images.get(index).ok_or_else(|| Error::Invalid(format!("no image {index}")))?;
        let row_bytes = meta.row_bytes();
        let (w, h) = (meta.width as usize, meta.height as usize);
        let start = self.pixel_base + meta.offset as usize;
        let data = Reader::at(&self.bmp, start).bytes(row_bytes * h)?;
        let mut out = Vec::with_capacity(w * h);
        for row in data.chunks_exact(row_bytes) {
            if meta.is_shadow() {
                out.extend((0..w).map(|x| (row[x / 8] >> (7 - x % 8)) & 1));
            } else {
                out.extend_from_slice(&row[..w]);
            }
        }
        Ok(out)
    }

    /// One image as RGBA (index 0 transparent; shadow pixels as `SHADOW_RGBA`).
    pub fn image_rgba(&self, index: usize, tint: Tint) -> Result<Image, Error> {
        let meta = self.art.images[index];
        let indices = self.image_indices(index)?;
        let palette = self.palette(&meta, tint);
        let mut rgba = Vec::with_capacity(indices.len() * 4);
        for &i in &indices {
            rgba.extend_from_slice(&pixel(&meta, &palette, i));
        }
        Ok(Image { width: meta.width, height: meta.height, rgba })
    }

    /// Bounds of a frame's layers in frame-local pixels: (min_x, min_y, max_x, max_y), max exclusive.
    pub fn frame_bounds(&self, frame: &Frame) -> (i32, i32, i32, i32) {
        let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for layer in &frame.layers {
            let meta = &self.art.images[layer.image as usize];
            let (x, y) = (layer.x as i32, layer.y as i32);
            b = (b.0.min(x), b.1.min(y), b.2.max(x + meta.width as i32), b.3.max(y + meta.height as i32));
        }
        if b.0 > b.2 { (0, 0, 0, 0) } else { b }
    }

    /// Union of `frame_bounds` over every frame of an animation.
    pub fn animation_bounds(&self, anim: &Animation) -> (i32, i32, i32, i32) {
        let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for f in anim.frames.iter().filter(|f| !f.layers.is_empty()) {
            let fb = self.frame_bounds(f);
            b = (b.0.min(fb.0), b.1.min(fb.1), b.2.max(fb.2), b.3.max(fb.3));
        }
        if b.0 > b.2 { (0, 0, 0, 0) } else { b }
    }

    /// Composites one frame (layers drawn in file order, later on top).
    pub fn frame_rgba(&self, anim: usize, frame: usize, tint: Tint) -> Result<Sprite, Error> {
        let a = self.animation(anim).ok_or_else(|| Error::Invalid(format!("no animation {anim}")))?;
        let f = a.frames.get(frame).ok_or_else(|| Error::Invalid(format!("animation {anim} has no frame {frame}")))?;
        let bounds = self.frame_bounds(f);
        self.compose(a, f, bounds, tint)
    }

    /// Like `frame_rgba` but on a canvas sized to the whole animation, so frames line up.
    pub fn frame_rgba_aligned(&self, anim: usize, frame: usize, tint: Tint) -> Result<Sprite, Error> {
        let a = self.animation(anim).ok_or_else(|| Error::Invalid(format!("no animation {anim}")))?;
        let f = a.frames.get(frame).ok_or_else(|| Error::Invalid(format!("animation {anim} has no frame {frame}")))?;
        self.compose(a, f, self.animation_bounds(a), tint)
    }

    /// The game's per-pixel click test on a frame (0x404F40, a structure's MouseOver): whether a click at `(x, y)`
    /// in animation coordinates (map pixel - object pixel + the animation's displacement) lands on a set byte of one
    /// of the frame's images. Faithful to its quirks: the image's right and bottom edges are inclusive (one byte past
    /// the row is read), and 1-bit shadow images are read as if they were 8-bit at their recorded stride, so a
    /// shadow is clickable in a pattern of stripes and dots, and next to it whatever bytes follow it in the file.
    /// An image whose type has bits 5-9 set (`k`) only counts inside rectangle `k` of the animation's appendix,
    /// less its displacement. `None` when the animation or frame doesn't exist.
    pub fn hit(&self, anim: usize, frame: usize, x: i32, y: i32) -> Option<bool> {
        let a = self.animation(anim)?;
        let f = a.frames.get(frame)?;
        let (dx, dy) = a.displacement;
        Some(f.layers.iter().any(|layer| {
            let meta = &self.art.images[layer.image as usize];
            let (ix, iy) = (x - layer.x as i32, y - layer.y as i32);
            let k = ((meta.kind & 0x3E0) >> 5) as usize;
            if k != 0 {
                let Some(r) = a.appendix.get(k - 1) else { return false };
                let r = r.map(|v| v as i32);
                if ix < r[0] - dx || ix > r[2] - dx || iy < r[1] - dy || iy > r[3] - dy {
                    return false;
                }
            }
            if ix < 0 || iy < 0 || ix > meta.width as i32 || iy > meta.height as i32 {
                return false;
            }
            let at = self.pixel_base + meta.offset as usize + iy as usize * meta.scanline_width as usize + ix as usize;
            self.bmp.get(at).is_some_and(|&b| b != 0)
        }))
    }

    /// An image whose type has bits 5-9 set (`k`) is drawn only inside rectangle `k` of the animation's appendix, in
    /// frame-local pixels, edges included (the sprite draw 0x403F20 narrows the surface's clip to it): how a Cargo
    /// Truck sinks into a Smelter's dock pit and a ConVec into a Structure Factory's.
    fn compose(&self, a: &Animation, f: &Frame, bounds: (i32, i32, i32, i32), tint: Tint) -> Result<Sprite, Error> {
        let (x0, y0, x1, y1) = bounds;
        let (width, height) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for layer in &f.layers {
            let meta = self.art.images[layer.image as usize];
            let palette = self.palette(&meta, tint);
            let indices = self.image_indices(layer.image as usize)?;
            let (ox, oy) = (layer.x as i32 - x0, layer.y as i32 - y0);
            let k = ((meta.kind & 0x3E0) >> 5) as usize;
            let clip = k.checked_sub(1).and_then(|k| a.appendix.get(k)).map(|r| r.map(|v| v as i32));
            for (i, &index) in indices.iter().enumerate() {
                let src = pixel(&meta, &palette, index);
                if src[3] == 0 {
                    continue;
                }
                let (lx, ly) = (layer.x as i32 + (i as u32 % meta.width) as i32, layer.y as i32 + (i as u32 / meta.width) as i32);
                if let Some(r) = clip
                    && (lx < r[0] || lx > r[2] || ly < r[1] || ly > r[3])
                {
                    continue;
                }
                let (x, y) = (ox + (i as u32 % meta.width) as i32, oy + (i as u32 / meta.width) as i32);
                let at = ((y as u32 * width + x as u32) * 4) as usize;
                blend(&mut rgba[at..at + 4], src);
            }
        }
        Ok(Sprite { image: Image { width, height, rgba }, hotspot: (a.displacement.0 - x0, a.displacement.1 - y0) })
    }
}

fn pixel(meta: &ImageMeta, palette: &Palette, index: u8) -> [u8; 4] {
    if meta.is_shadow() {
        if index != 0 { SHADOW_RGBA } else { [0; 4] }
    } else if index == 0 {
        [0; 4]
    } else {
        let [r, g, b] = palette[index as usize];
        [r, g, b, 255]
    }
}

/// Source-over blend of `src` onto `dst` (straight alpha).
pub(crate) fn blend(dst: &mut [u8], src: [u8; 4]) {
    if src[3] == 255 || dst[3] == 0 {
        dst.copy_from_slice(&src);
        return;
    }
    let (sa, da) = (src[3] as u32, dst[3] as u32);
    let out_a = sa + da * (255 - sa) / 255;
    for c in 0..3 {
        dst[c] = ((src[c] as u32 * sa + dst[c] as u32 * da * (255 - sa) / 255) / out_a) as u8;
    }
    dst[3] = out_a as u8;
}

fn parse_color_bmp(data: &[u8]) -> Result<Vec<[u8; 3]>, Error> {
    if data.get(..2) != Some(b"BM") {
        return Err(Error::Invalid("color.bmp is not a BMP file".into()));
    }
    let info_size = Reader::at(data, 14).u32()? as usize;
    let mut r = Reader::at(data, 14 + info_size);
    let mut colors = Vec::new();
    for _ in 0..PLAYER_COLORS * PLAYER_COLOR_RAMP {
        let bgrx = r.bytes(4)?;
        colors.push([bgrx[2], bgrx[1], bgrx[0]]);
    }
    Ok(colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-palette, two-image, one-animation art file plus its BMP pixel store.
    fn synthetic() -> (Vec<u8>, Vec<u8>) {
        let mut prt = Vec::new();
        let put = |v: &mut Vec<u8>, b: &[u8]| v.extend_from_slice(b);
        put(&mut prt, b"CPAL");
        put(&mut prt, &1u32.to_le_bytes());
        put(&mut prt, b"PPAL");
        put(&mut prt, &1048u32.to_le_bytes());
        put(&mut prt, b"head");
        put(&mut prt, &4u32.to_le_bytes());
        put(&mut prt, &1u32.to_le_bytes());
        put(&mut prt, b"data");
        put(&mut prt, &1024u32.to_le_bytes());
        for i in 0..256u32 {
            put(&mut prt, &[i as u8, 10, 20, 4]); // stored R, G, B, flags
        }
        put(&mut prt, &2u32.to_le_bytes());
        // 8-bit 3x2 image at offset 0 (rows padded to 4), 1-bit 9x2 shadow at offset 8.
        for (stride, offset, h, w, kind) in [(4u32, 0u32, 2u32, 3u32, 1u16), (12, 8, 2, 9, 5)] {
            for v in [stride, offset, h, w] {
                put(&mut prt, &v.to_le_bytes());
            }
            put(&mut prt, &kind.to_le_bytes());
            put(&mut prt, &0u16.to_le_bytes());
        }
        for v in [1u32, 1, 2, 0] {
            put(&mut prt, &v.to_le_bytes()); // animations, frames, layers, optional count
        }
        for v in [1i32, 0, 0, 10, 10, 5, 4, 0, 1] {
            put(&mut prt, &v.to_le_bytes()); // unknown, rect, displacement, unknown2, frames
        }
        put(&mut prt, &[0x82, 0, 3u8, 0xFE]); // 2 layers + optional pair (3, -2)
        for (image, x, y) in [(1u16, -1i16, 0i16), (0, 2, 1)] {
            put(&mut prt, &image.to_le_bytes());
            put(&mut prt, &[255, 0]);
            put(&mut prt, &x.to_le_bytes());
            put(&mut prt, &y.to_le_bytes());
        }
        put(&mut prt, &0u32.to_le_bytes());

        let mut bmp = vec![0u8; 14];
        bmp[..2].copy_from_slice(b"BM");
        bmp[10..14].copy_from_slice(&14u32.to_le_bytes());
        bmp.extend_from_slice(&[0, 7, 9, 0, 1, 2, 3, 0]); // 8-bit rows
        bmp.extend_from_slice(&[0b1000_0000, 0b1000_0000, 0, 0, 0xFF, 0x80, 0, 0]); // 1-bit rows
        (prt, bmp)
    }

    #[test]
    fn parses_and_composites_synthetic_art() {
        let (prt, bmp) = synthetic();
        let art = ArtFile::parse(&prt).unwrap();
        assert_eq!(art.palettes[0][7], [7, 10, 20]);
        assert_eq!(art.animations[0].frames[0].optional1, Some([3, -2]));
        let sprites = Sprites::new(art, bmp, None).unwrap();
        assert_eq!(sprites.image_indices(0).unwrap(), vec![0, 7, 9, 1, 2, 3]);
        assert_eq!(sprites.image_indices(1).unwrap(), [vec![1, 0, 0, 0, 0, 0, 0, 0, 1], vec![1; 9]].concat());

        let sprite = sprites.frame_rgba(0, 0, Tint::Stored).unwrap();
        // Layers span x -1..8, y 0..3; displacement (5, 4) -> hotspot (6, 4).
        assert_eq!((sprite.image.width, sprite.image.height), (9, 3));
        assert_eq!(sprite.hotspot, (6, 4));
        let px = |x: u32, y: u32| &sprite.image.rgba[((y * 9 + x) * 4) as usize..][..4];
        assert_eq!(px(0, 0), &SHADOW_RGBA); // shadow pixel
        assert_eq!(px(1, 0), &[0, 0, 0, 0]); // shadow clear
        assert_eq!(px(4, 1), &[7, 10, 20, 255]); // 8-bit pixel over shadow row 0
        assert_eq!(px(3, 1), &SHADOW_RGBA); // index 0 is transparent: the shadow shows through
        assert_eq!(px(5, 2), &[3, 10, 20, 255]);
    }

    /// The Cargo Truck unloading at a Smelter (animation 497, harness eco_mining): images with a clip index in their
    /// type (bits 5-9) stay inside the animation's appendix rectangle, so the truck sinks into the dock pit instead
    /// of showing below it.
    #[test]
    fn clipped_layers_stay_in_the_dock_pit() {
        let Some(files) = crate::testdata::game_files() else { return };
        let sprites = Sprites::load(&files).unwrap();
        let a = sprites.animation(497).unwrap();
        let r = a.appendix[1];
        let s = sprites.frame_rgba_aligned(497, 110, Tint::Stored).unwrap();
        let (x0, y0) = (a.displacement.0 - s.hotspot.0, a.displacement.1 - s.hotspot.1);
        // The truck (image 254, 12x33 at (21, 30)) would reach y 62; nothing of it is drawn below the pit.
        // (The pit images themselves end at y 40.)
        for y in (r[3] as i32 + 1).max(40)..30 + 33 {
            for x in 21..33 {
                let (cx, cy) = (x - x0, y - y0);
                if cx < 0 || cy < 0 || cx >= s.image.width as i32 || cy >= s.image.height as i32 {
                    continue;
                }
                let at = ((cy as u32 * s.image.width + cx as u32) * 4) as usize;
                assert!(s.image.rgba[at + 3] < 255, "opaque truck pixel at frame-local ({x}, {y})");
            }
        }
    }
}
