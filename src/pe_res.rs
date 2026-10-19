//! Windows PE resources (`.rsrc`): the directory tree and decoders for the resource types the
//! game's DLLs use (bitmaps, string tables, dialogs, menus, cursors and icons, version info).
//!
//! The game keeps nearly all of its UI art in `out2res.dll` (in-game) and `op2shres.dll` (the
//! shell menus) as ordinary Win32 resources, so this is a plain reader with no Outpost 2
//! specifics. Input is untrusted: every offset is bounds-checked and errors are values.

use crate::Error;
use crate::reader::Reader;

pub const RT_CURSOR: u16 = 1;
pub const RT_BITMAP: u16 = 2;
pub const RT_ICON: u16 = 3;
pub const RT_MENU: u16 = 4;
pub const RT_DIALOG: u16 = 5;
pub const RT_STRING: u16 = 6;
pub const RT_FONTDIR: u16 = 7;
pub const RT_FONT: u16 = 8;
pub const RT_ACCELERATOR: u16 = 9;
pub const RT_RCDATA: u16 = 10;
pub const RT_GROUP_CURSOR: u16 = 12;
pub const RT_GROUP_ICON: u16 = 14;
pub const RT_VERSION: u16 = 16;

/// A resource type or name: numeric id or a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResId {
    Id(u16),
    Name(String),
}

impl ResId {
    /// Parses a CLI-style id: a number, a standard type name (`BITMAP`, `RT_STRING`...), or a name.
    pub fn parse_type(s: &str) -> Self {
        let upper = s.to_ascii_uppercase();
        let bare = upper.strip_prefix("RT_").unwrap_or(&upper);
        if let Some(id) = (1..=24).find(|&id| type_name(id) == Some(bare)) {
            return ResId::Id(id);
        }
        Self::parse(s)
    }

    pub fn parse(s: &str) -> Self {
        s.parse().map(ResId::Id).unwrap_or_else(|_| ResId::Name(s.to_string()))
    }
}

impl std::fmt::Display for ResId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResId::Id(id) => write!(f, "{id}"),
            ResId::Name(n) => write!(f, "\"{n}\""),
        }
    }
}

/// The standard name of a numeric resource type.
pub fn type_name(id: u16) -> Option<&'static str> {
    Some(match id {
        1 => "CURSOR",
        2 => "BITMAP",
        3 => "ICON",
        4 => "MENU",
        5 => "DIALOG",
        6 => "STRING",
        7 => "FONTDIR",
        8 => "FONT",
        9 => "ACCELERATOR",
        10 => "RCDATA",
        11 => "MESSAGETABLE",
        12 => "GROUP_CURSOR",
        14 => "GROUP_ICON",
        16 => "VERSION",
        17 => "DLGINCLUDE",
        19 => "PLUGPLAY",
        20 => "VXD",
        21 => "ANICURSOR",
        22 => "ANIICON",
        23 => "HTML",
        24 => "MANIFEST",
        _ => return None,
    })
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub kind: ResId,
    pub name: ResId,
    pub lang: u16,
    pub codepage: u32,
    /// File offset and length of the data.
    pub offset: usize,
    pub len: usize,
}

/// The resources of one PE file (PE32 or PE32+), with the file bytes they point into.
pub struct PeResources {
    data: Vec<u8>,
    pub resources: Vec<Resource>,
}

struct Section {
    rva: u32,
    vsize: u32,
    raw_offset: u32,
    raw_size: u32,
}

impl PeResources {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let mut r = Reader::new(&data);
        if r.bytes(2)? != b"MZ" {
            return Err(Error::Invalid("not a PE file (no MZ header)".into()));
        }
        let pe = Reader::at(&data, 0x3c).u32()? as usize;
        let mut r = Reader::at(&data, pe);
        r.expect_tag(b"PE\0\0")?;
        r.skip(2)?; // machine
        let nsections = r.u16()? as usize;
        r.skip(12)?;
        let opt_size = r.u16()? as usize;
        r.skip(2)?;
        let opt = r.pos();
        let magic = r.u16()?;
        let dirs = match magic {
            0x10b => opt + 96,
            0x20b => opt + 112,
            _ => return Err(Error::Invalid(format!("unknown optional header magic {magic:#x}"))),
        };
        let ndirs = Reader::at(&data, dirs - 4).u32()?;
        let (rsrc_rva, rsrc_size) = if ndirs > 2 {
            let mut d = Reader::at(&data, dirs + 2 * 8);
            (d.u32()?, d.u32()?)
        } else {
            (0, 0)
        };
        let mut sections = Vec::new();
        let mut s = Reader::at(&data, opt + opt_size);
        for _ in 0..nsections {
            s.skip(8)?;
            let vsize = s.u32()?;
            let rva = s.u32()?;
            let raw_size = s.u32()?;
            let raw_offset = s.u32()?;
            s.skip(16)?;
            sections.push(Section { rva, vsize, raw_offset, raw_size });
        }
        let mut out = Self { data, resources: Vec::new() };
        if rsrc_rva == 0 || rsrc_size == 0 {
            return Ok(out);
        }
        let to_offset = |rva: u32| -> Option<usize> {
            sections.iter().find_map(|s| {
                let size = s.vsize.max(s.raw_size);
                (rva >= s.rva && rva < s.rva + size && rva - s.rva < s.raw_size)
                    .then(|| (s.raw_offset + (rva - s.rva)) as usize)
            })
        };
        let base = to_offset(rsrc_rva).ok_or_else(|| Error::Invalid("resource directory outside sections".into()))?;
        let mut resources = Vec::new();
        let data = &out.data;
        // Three fixed levels: type, name, language.
        for (kind, name_dir) in read_dir(data, base, 0)? {
            let Dir::Sub(name_dir) = name_dir else { continue };
            for (name, lang_dir) in read_dir(data, base, name_dir)? {
                let Dir::Sub(lang_dir) = lang_dir else { continue };
                for (lang, leaf) in read_dir(data, base, lang_dir)? {
                    let Dir::Leaf(entry) = leaf else { continue };
                    let mut e = Reader::at(data, base + entry);
                    let rva = e.u32()?;
                    let len = e.u32()? as usize;
                    let codepage = e.u32()?;
                    let Some(offset) = to_offset(rva) else { continue };
                    if offset.checked_add(len).is_none_or(|end| end > data.len()) {
                        return Err(Error::Invalid(format!("resource {kind}/{name} runs past the end of the file")));
                    }
                    let lang = match lang {
                        ResId::Id(l) => l,
                        ResId::Name(_) => 0,
                    };
                    resources.push(Resource { kind: kind.clone(), name: name.clone(), lang, codepage, offset, len });
                }
            }
        }
        out.resources = resources;
        Ok(out)
    }

    pub fn bytes(&self, res: &Resource) -> &[u8] {
        &self.data[res.offset..res.offset + res.len]
    }

    pub fn find(&self, kind: &ResId, name: &ResId) -> Option<&Resource> {
        self.resources.iter().find(|r| &r.kind == kind && name_eq(&r.name, name))
    }

    pub fn of_type<'a>(&'a self, kind: &'a ResId) -> impl Iterator<Item = &'a Resource> + 'a {
        self.resources.iter().filter(move |r| &r.kind == kind)
    }

    /// A bitmap resource decoded to RGBA.
    pub fn bitmap(&self, name: &ResId) -> Result<Dib, Error> {
        let res = self.find(&ResId::Id(RT_BITMAP), name).ok_or_else(|| Error::NotFound(format!("bitmap {name}")))?;
        Dib::parse(self.bytes(res))
    }

    /// A dialog template by numeric id.
    pub fn dialog(&self, id: u16) -> Result<Dialog, Error> {
        let res = self.find(&ResId::Id(RT_DIALOG), &ResId::Id(id)).ok_or_else(|| Error::NotFound(format!("dialog {id}")))?;
        Dialog::parse(self.bytes(res))
    }

    /// Every string in the string tables, as (id, text), sorted by id.
    pub fn strings(&self) -> Result<Vec<(u32, String)>, Error> {
        let mut out = Vec::new();
        for res in self.of_type(&ResId::Id(RT_STRING)) {
            let ResId::Id(block) = res.name else { continue };
            let mut r = Reader::new(self.bytes(res));
            for i in 0..16u32 {
                let len = r.u16()? as usize;
                if len > 0 {
                    let text = utf16(r.bytes(len * 2)?);
                    out.push(((block as u32 - 1) * 16 + i, text));
                }
            }
        }
        out.sort_by_key(|s| s.0);
        Ok(out)
    }

    /// A cursor (`RT_GROUP_CURSOR` name) or icon (`RT_GROUP_ICON` name): the largest image.
    pub fn cursor_or_icon(&self, group_type: u16, name: &ResId) -> Result<CursorImage, Error> {
        let group = self
            .find(&ResId::Id(group_type), name)
            .ok_or_else(|| Error::NotFound(format!("{} {name}", type_name(group_type).unwrap_or("?"))))?;
        let mut r = Reader::new(self.bytes(group));
        r.skip(4)?;
        let count = r.u16()?;
        let mut best: Option<(u32, u16)> = None;
        for _ in 0..count {
            let w = r.bytes(1)?[0] as u32;
            r.skip(11)?;
            let id = r.u16()?;
            let area = if w == 0 { 256 } else { w };
            if best.is_none_or(|(a, _)| area > a) {
                best = Some((area, id));
            }
        }
        let (_, id) = best.ok_or_else(|| Error::Invalid("empty cursor/icon group".into()))?;
        let image_type = if group_type == RT_GROUP_CURSOR { RT_CURSOR } else { RT_ICON };
        let res = self
            .find(&ResId::Id(image_type), &ResId::Id(id))
            .ok_or_else(|| Error::NotFound(format!("image {id} of group {name}")))?;
        let bytes = self.bytes(res);
        if image_type == RT_CURSOR {
            let mut r = Reader::new(bytes);
            let hx = r.u16()?;
            let hy = r.u16()?;
            Ok(CursorImage { hotspot: (hx, hy), image: Dib::parse_icon(&bytes[4..])? })
        } else {
            Ok(CursorImage { hotspot: (0, 0), image: Dib::parse_icon(bytes)? })
        }
    }
}

fn name_eq(a: &ResId, b: &ResId) -> bool {
    match (a, b) {
        (ResId::Name(a), ResId::Name(b)) => a.eq_ignore_ascii_case(b),
        _ => a == b,
    }
}

enum Dir {
    Sub(usize),
    Leaf(usize),
}

fn read_dir(data: &[u8], base: usize, offset: usize) -> Result<Vec<(ResId, Dir)>, Error> {
    let mut r = Reader::at(data, base + offset);
    r.skip(12)?;
    let named = r.u16()? as usize;
    let ids = r.u16()? as usize;
    let mut out = Vec::new();
    for _ in 0..named + ids {
        let name = r.u32()?;
        let target = r.u32()?;
        let id = if name & 0x8000_0000 != 0 {
            let mut s = Reader::at(data, base + (name & 0x7fff_ffff) as usize);
            let len = s.u16()? as usize;
            ResId::Name(utf16(s.bytes(len * 2)?))
        } else {
            ResId::Id(name as u16)
        };
        let target_off = (target & 0x7fff_ffff) as usize;
        // A directory pointing at or before itself would loop forever.
        if target & 0x8000_0000 != 0 {
            if target_off <= offset {
                return Err(Error::Invalid("resource directory loop".into()));
            }
            out.push((id, Dir::Sub(target_off)));
        } else {
            out.push((id, Dir::Leaf(target_off)));
        }
    }
    Ok(out)
}

pub(crate) fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    String::from_utf16_lossy(&units)
}

/// A device-independent bitmap, decoded.
#[derive(Debug, Clone)]
pub struct Dib {
    pub width: u32,
    pub height: u32,
    pub bit_count: u16,
    pub compression: u32,
    /// Palette as RGB (empty for true-colour bitmaps).
    pub palette: Vec<[u8; 3]>,
    /// Palette indices, top row first (only for <= 8 bits per pixel).
    pub indices: Vec<u8>,
    /// RGBA, top row first.
    pub rgba: Vec<u8>,
}

impl Dib {
    /// Parses a BITMAPINFOHEADER-led DIB (a bitmap resource, or a `.bmp` minus its file header).
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        Self::parse_inner(data, false)
    }

    /// Parses an icon/cursor image: a DIB whose height covers the XOR image plus an AND mask.
    pub fn parse_icon(data: &[u8]) -> Result<Self, Error> {
        if data.starts_with(b"\x89PNG") {
            return Err(Error::Unsupported("PNG icon images".into()));
        }
        Self::parse_inner(data, true)
    }

    fn parse_inner(data: &[u8], icon: bool) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        let header_size = r.u32()? as usize;
        if header_size < 40 {
            return Err(Error::Unsupported(format!("DIB header size {header_size} (OS/2 bitmaps)")));
        }
        let width = r.i32()?;
        let raw_height = r.i32()?;
        r.skip(2)?;
        let bit_count = r.u16()?;
        let compression = r.u32()?;
        r.skip(12)?;
        let colors_used = r.u32()? as usize;
        let top_down = raw_height < 0;
        let mut height = raw_height.unsigned_abs();
        if icon {
            height /= 2;
        }
        if width <= 0 || width > 16384 || height == 0 || height > 16384 {
            return Err(Error::Invalid(format!("DIB size {width}x{raw_height}")));
        }
        let width = width as u32;
        let ncolors = match (bit_count, colors_used) {
            (1 | 4 | 8, 0) => 1usize << bit_count,
            (1 | 4 | 8, n) => n.min(256),
            (_, n) => n.min(256),
        };
        let mut masks = None;
        let mut pos = header_size;
        if compression == 3 && header_size == 40 {
            let mut m = Reader::at(data, pos);
            masks = Some([m.u32()?, m.u32()?, m.u32()?]);
            pos += 12;
        } else if compression == 3 {
            let mut m = Reader::at(data, 40);
            masks = Some([m.u32()?, m.u32()?, m.u32()?]);
        }
        let mut palette = Vec::with_capacity(ncolors);
        let mut p = Reader::at(data, pos);
        for _ in 0..ncolors {
            let c = p.bytes(4)?;
            palette.push([c[2], c[1], c[0]]);
        }
        pos += ncolors * 4;
        let pixels = data.get(pos..).unwrap_or(&[]);
        let (w, h) = (width as usize, height as usize);
        let row_of = |y: usize| if top_down { y } else { h - 1 - y };

        let mut indices = Vec::new();
        let mut rgba = vec![0u8; w * h * 4];
        match (bit_count, compression) {
            (1 | 4 | 8, 0) => {
                let stride = (w * bit_count as usize).div_ceil(32) * 4;
                if pixels.len() < stride * h {
                    return Err(Error::Truncated { offset: pos, wanted: stride * h });
                }
                indices = vec![0u8; w * h];
                for y in 0..h {
                    let src = &pixels[row_of(y) * stride..][..stride];
                    for x in 0..w {
                        indices[y * w + x] = match bit_count {
                            8 => src[x],
                            4 => (src[x / 2] >> (4 - 4 * (x % 2))) & 0xf,
                            _ => (src[x / 8] >> (7 - x % 8)) & 1,
                        };
                    }
                }
            }
            (8, 1) | (4, 2) => {
                indices = decode_rle(pixels, w, h, bit_count == 4)?;
                if !top_down {
                    let mut flipped = vec![0u8; w * h];
                    for y in 0..h {
                        flipped[y * w..][..w].copy_from_slice(&indices[(h - 1 - y) * w..][..w]);
                    }
                    indices = flipped;
                }
            }
            (16 | 24 | 32, 0 | 3) => {
                let bpp = bit_count as usize / 8;
                let stride = (w * bpp).div_ceil(4) * 4;
                if pixels.len() < stride * h {
                    return Err(Error::Truncated { offset: pos, wanted: stride * h });
                }
                let masks = masks.unwrap_or(match bit_count {
                    16 => [0x7c00, 0x03e0, 0x001f],
                    _ => [0xff0000, 0xff00, 0xff],
                });
                for y in 0..h {
                    let src = &pixels[row_of(y) * stride..];
                    for x in 0..w {
                        let px = &src[x * bpp..][..bpp];
                        let v = match bpp {
                            2 => u16::from_le_bytes([px[0], px[1]]) as u32,
                            3 => u32::from_le_bytes([px[0], px[1], px[2], 0]),
                            _ => u32::from_le_bytes([px[0], px[1], px[2], px[3]]),
                        };
                        let o = (y * w + x) * 4;
                        for (c, &m) in masks.iter().enumerate() {
                            rgba[o + c] = scale_mask(v, m);
                        }
                        rgba[o + 3] = 255;
                    }
                }
            }
            _ => {
                return Err(Error::Unsupported(format!("DIB with {bit_count} bpp, compression {compression}")));
            }
        }
        if !indices.is_empty() {
            for (i, &ix) in indices.iter().enumerate() {
                let c = palette.get(ix as usize).copied().unwrap_or([0, 0, 0]);
                rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        if icon {
            // The AND mask follows the XOR image: 1 bit per pixel, 1 = transparent.
            let xor_stride = (w * bit_count as usize).div_ceil(32) * 4;
            let mask_start = pos + xor_stride * h;
            let mask_stride = w.div_ceil(32) * 4;
            if let Some(mask) = data.get(mask_start..mask_start + mask_stride * h) {
                for y in 0..h {
                    let row = &mask[row_of(y) * mask_stride..];
                    for x in 0..w {
                        if (row[x / 8] >> (7 - x % 8)) & 1 == 1 {
                            rgba[(y * w + x) * 4 + 3] = 0;
                        }
                    }
                }
            }
        }
        Ok(Self { width, height, bit_count, compression, palette, indices, rgba })
    }
}

fn scale_mask(v: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let bits = (mask >> shift).count_ones();
    let c = (v & mask) >> shift;
    ((c * 255 + ((1 << bits) - 1) / 2) / ((1 << bits) - 1)) as u8
}

/// BI_RLE8 / BI_RLE4, rows in file order (bottom-up unless the caller says otherwise).
fn decode_rle(src: &[u8], w: usize, h: usize, four: bool) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; w * h];
    let (mut x, mut y) = (0usize, 0usize);
    let mut i = 0;
    let mut put = |x: &mut usize, y: usize, v: u8| {
        if *x < w && y < h {
            out[y * w + *x] = v;
        }
        *x += 1;
    };
    while i + 1 < src.len() {
        let (n, v) = (src[i] as usize, src[i + 1]);
        i += 2;
        if n > 0 {
            for k in 0..n {
                let px = if four { if k % 2 == 0 { v >> 4 } else { v & 0xf } } else { v };
                put(&mut x, y, px);
            }
            continue;
        }
        match v {
            0 => {
                x = 0;
                y += 1;
            }
            1 => break,
            2 => {
                let d = src.get(i..i + 2).ok_or(Error::Truncated { offset: i, wanted: 2 })?;
                x += d[0] as usize;
                y += d[1] as usize;
                i += 2;
            }
            count => {
                let count = count as usize;
                let bytes = if four { count.div_ceil(2) } else { count };
                let run = src.get(i..i + bytes).ok_or(Error::Truncated { offset: i, wanted: bytes })?;
                for k in 0..count {
                    let px = if four { if k % 2 == 0 { run[k / 2] >> 4 } else { run[k / 2] & 0xf } } else { run[k] };
                    put(&mut x, y, px);
                }
                i += bytes + (bytes & 1);
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct CursorImage {
    pub hotspot: (u16, u16),
    pub image: Dib,
}

/// An animated cursor (`.ani`, RIFF `ACON`).
#[derive(Debug, Clone)]
pub struct AniCursor {
    pub frames: Vec<CursorImage>,
    /// Display rate per step in jiffies (1/60 s); `seq` gives the frame per step.
    pub rates: Vec<u32>,
    pub seq: Vec<u32>,
    pub default_rate: u32,
    pub title: Option<String>,
}

impl AniCursor {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        r.expect_tag(b"RIFF")?;
        r.u32()?;
        r.expect_tag(b"ACON")?;
        let mut ani = AniCursor { frames: Vec::new(), rates: Vec::new(), seq: Vec::new(), default_rate: 0, title: None };
        walk_riff(data, r.pos(), data.len(), &mut ani)?;
        if ani.frames.is_empty() {
            return Err(Error::Invalid("ani cursor has no frames".into()));
        }
        Ok(ani)
    }
}

fn walk_riff(data: &[u8], mut pos: usize, end: usize, ani: &mut AniCursor) -> Result<(), Error> {
    while pos + 8 <= end {
        let mut r = Reader::at(data, pos);
        let tag = r.tag()?;
        let len = r.u32()? as usize;
        let body = pos + 8;
        let body_end = body.checked_add(len).filter(|&e| e <= data.len()).ok_or(Error::Truncated { offset: body, wanted: len })?;
        let chunk = &data[body..body_end];
        match &tag {
            b"LIST" => walk_riff(data, body + 4, body_end, ani)?,
            b"anih" => {
                let mut a = Reader::new(chunk);
                a.skip(28)?;
                ani.default_rate = a.u32()?;
            }
            b"rate" => ani.rates = chunk.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect(),
            b"seq " => ani.seq = chunk.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect(),
            b"INAM" => ani.title = Some(String::from_utf8_lossy(chunk).trim_end_matches('\0').to_string()),
            b"icon" => ani.frames.push(parse_cur_file(chunk)?),
            _ => {}
        }
        pos = body_end + (len & 1);
    }
    Ok(())
}

/// A `.cur`/`.ico` file: the first (largest-listed) image.
pub fn parse_cur_file(data: &[u8]) -> Result<CursorImage, Error> {
    let mut r = Reader::new(data);
    r.skip(2)?;
    let kind = r.u16()?;
    let count = r.u16()?;
    if count == 0 {
        return Err(Error::Invalid("empty icon file".into()));
    }
    r.skip(4)?;
    let hx = r.u16()?;
    let hy = r.u16()?;
    let size = r.u32()? as usize;
    let offset = r.u32()? as usize;
    let img = data.get(offset..offset + size).ok_or(Error::Truncated { offset, wanted: size })?;
    let hotspot = if kind == 2 { (hx, hy) } else { (0, 0) };
    Ok(CursorImage { hotspot, image: Dib::parse_icon(img)? })
}

/// A dialog template (`DLGTEMPLATE` or `DLGTEMPLATEEX`), in dialog units.
#[derive(Debug, Clone)]
pub struct Dialog {
    pub style: u32,
    pub rect: [i16; 4],
    pub menu: Option<ResId>,
    pub class: Option<ResId>,
    pub title: String,
    pub font: Option<(u16, String)>,
    pub controls: Vec<DialogControl>,
}

#[derive(Debug, Clone)]
pub struct DialogControl {
    pub id: u32,
    pub style: u32,
    /// x, y, width, height in dialog units.
    pub rect: [i16; 4],
    /// Window class: a predefined atom (0x80 button, 0x81 edit, 0x82 static, 0x83 listbox,
    /// 0x84 scrollbar, 0x85 combobox) or a registered class name.
    pub class: ResId,
    pub text: ResId,
}

impl DialogControl {
    pub fn class_name(&self) -> String {
        match &self.class {
            ResId::Id(0x80) => "BUTTON".into(),
            ResId::Id(0x81) => "EDIT".into(),
            ResId::Id(0x82) => "STATIC".into(),
            ResId::Id(0x83) => "LISTBOX".into(),
            ResId::Id(0x84) => "SCROLLBAR".into(),
            ResId::Id(0x85) => "COMBOBOX".into(),
            ResId::Id(n) => format!("#{n:#x}"),
            ResId::Name(n) => n.clone(),
        }
    }
}

/// A word-aligned "sz_Or_Ord": 0 = none, 0xFFFF = ordinal, else a NUL-terminated UTF-16 string.
fn sz_or_ord(r: &mut Reader) -> Result<Option<ResId>, Error> {
    let first = r.u16()?;
    match first {
        0 => Ok(None),
        0xffff => Ok(Some(ResId::Id(r.u16()?))),
        _ => {
            let mut units = vec![first];
            loop {
                let u = r.u16()?;
                if u == 0 {
                    break;
                }
                units.push(u);
                if units.len() > 4096 {
                    return Err(Error::Invalid(format!("unterminated string at {:#x}", r.pos())));
                }
            }
            Ok(Some(ResId::Name(String::from_utf16_lossy(&units))))
        }
    }
}

fn sz(r: &mut Reader) -> Result<String, Error> {
    Ok(match sz_or_ord(r)? {
        Some(ResId::Name(s)) => s,
        Some(ResId::Id(id)) => format!("#{id}"),
        None => String::new(),
    })
}

impl Dialog {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        let ex = data.len() >= 4 && data[0..4] == [1, 0, 0xff, 0xff];
        let style;
        if ex {
            r.skip(8)?; // version, signature, helpID
            r.u32()?; // exStyle
            style = r.u32()?;
        } else {
            style = r.u32()?;
            r.u32()?;
        }
        let count = r.u16()? as usize;
        let rect = [r.u16()? as i16, r.u16()? as i16, r.u16()? as i16, r.u16()? as i16];
        let menu = sz_or_ord(&mut r)?;
        let class = sz_or_ord(&mut r)?;
        let title = sz(&mut r)?;
        let mut font = None;
        if style & 0x40 != 0 {
            // DS_SETFONT
            let size = r.u16()?;
            if ex {
                r.skip(4)?; // weight, italic, charset
            }
            font = Some((size, sz(&mut r)?));
        }
        let mut controls = Vec::new();
        for _ in 0..count {
            let aligned = r.pos().div_ceil(4) * 4;
            r = Reader::at(data, aligned);
            let (id, style);
            if ex {
                r.u32()?; // helpID
                r.u32()?; // exStyle
                style = r.u32()?;
            } else {
                style = r.u32()?;
                r.u32()?;
            }
            let rect = [r.u16()? as i16, r.u16()? as i16, r.u16()? as i16, r.u16()? as i16];
            id = if ex { r.u32()? } else { r.u16()? as u32 };
            let class = sz_or_ord(&mut r)?.unwrap_or(ResId::Id(0));
            let text = sz_or_ord(&mut r)?.unwrap_or(ResId::Name(String::new()));
            let extra = r.u16()? as usize;
            r.skip(extra)?;
            controls.push(DialogControl { id, style, rect, class, text });
        }
        Ok(Self { style, rect, menu, class, title, font, controls })
    }
}

/// A menu template (standard, not MENUEX), flattened: (depth, id, text).
pub fn parse_menu(data: &[u8]) -> Result<Vec<(usize, u16, String)>, Error> {
    let mut r = Reader::new(data);
    let version = r.u16()?;
    if version != 0 {
        return Err(Error::Unsupported("MENUEX templates".into()));
    }
    let header = r.u16()? as usize;
    r.skip(header)?;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut ends = Vec::new();
    while r.pos() + 2 <= data.len() {
        let flags = r.u16()?;
        let popup = flags & 0x10 != 0;
        let id = if popup { 0 } else { r.u16()? };
        let text = sz(&mut r)?;
        out.push((depth, id, text));
        if popup {
            ends.push(flags & 0x80 != 0);
            depth += 1;
        } else if flags & 0x80 != 0 {
            // MF_END closes this level, and any parents whose last item was this popup.
            while let Some(parent_end) = ends.pop() {
                depth -= 1;
                if !parent_end {
                    break;
                }
            }
            if depth == 0 && ends.is_empty() {
                break;
            }
        }
    }
    Ok(out)
}

/// The fixed and string fields of a VS_VERSIONINFO resource.
pub fn version_strings(data: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // StringFileInfo children are (wLength, wValueLength, wType, key, pad, value) records; scan
    // for them leniently, since only the key/value pairs are interesting.
    let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let mut i = 0;
    while i + 3 < units.len() {
        let (len, vlen, kind) = (units[i] as usize, units[i + 1] as usize, units[i + 2]);
        if kind == 1 && len >= 8 && vlen > 0 && i * 2 + len <= data.len() {
            let key_start = i + 3;
            if let Some(key_len) = units[key_start..].iter().position(|&u| u == 0) {
                let key = String::from_utf16_lossy(&units[key_start..key_start + key_len]);
                let mut v = key_start + key_len + 1;
                if v % 2 == 1 {
                    v += 1;
                }
                if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric()) && v + vlen <= units.len() {
                    let value: String =
                        String::from_utf16_lossy(&units[v..v + vlen]).trim_end_matches('\0').to_string();
                    if !matches!(key.as_str(), "StringFileInfo" | "VarFileInfo" | "Translation") {
                        out.push((key, value));
                    }
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dib_header(w: i32, h: i32, bits: u16, compression: u32, colors: u32) -> Vec<u8> {
        let mut d = Vec::new();
        for v in [40u32, w as u32, h as u32] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&bits.to_le_bytes());
        for v in [compression, 0, 0, 0, colors, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        for i in 0..colors {
            d.extend_from_slice(&[i as u8, 0, 255 - i as u8, 0]); // B, G, R, 0
        }
        d
    }

    #[test]
    fn rle8_bottom_up() {
        // 4x2: bottom row "run of 4 x index 1", top row "absolute 2,3,2 then index 0".
        let mut d = dib_header(4, 2, 8, 1, 4);
        d.extend_from_slice(&[4, 1, 0, 0, 0, 3, 2, 3, 2, 0, 1, 0, 0, 0, 0, 1]);
        let dib = Dib::parse(&d).unwrap();
        assert_eq!(dib.indices, vec![2, 3, 2, 0, 1, 1, 1, 1]);
        assert_eq!(&dib.rgba[0..4], &[253, 0, 2, 255]); // palette entry 2 as RGB
    }

    #[test]
    fn dialog_template() {
        let mut d = Vec::new();
        let w16 = |d: &mut Vec<u8>, v: u16| d.extend_from_slice(&v.to_le_bytes());
        d.extend_from_slice(&0x80c0_0040u32.to_le_bytes()); // style with DS_SETFONT
        d.extend_from_slice(&0u32.to_le_bytes());
        w16(&mut d, 1);
        for v in [0, 0, 124, 196] {
            w16(&mut d, v);
        }
        w16(&mut d, 0); // no menu
        w16(&mut d, 0); // default class
        for c in "MENU".encode_utf16().chain([0]) {
            w16(&mut d, c);
        }
        w16(&mut d, 8);
        for c in "MS Sans Serif".encode_utf16().chain([0]) {
            w16(&mut d, c);
        }
        while d.len() % 4 != 0 {
            d.push(0);
        }
        d.extend_from_slice(&0x5001_0000u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        for v in [6, 7, 53, 14, 1, 0xffff, 0x80] {
            w16(&mut d, v);
        }
        for c in "OK".encode_utf16().chain([0]) {
            w16(&mut d, c);
        }
        w16(&mut d, 0);
        let dlg = Dialog::parse(&d).unwrap();
        assert_eq!(dlg.title, "MENU");
        assert_eq!(dlg.font, Some((8, "MS Sans Serif".to_string())));
        assert_eq!(dlg.controls.len(), 1);
        assert_eq!(dlg.controls[0].class_name(), "BUTTON");
        assert_eq!(dlg.controls[0].rect, [6, 7, 53, 14]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(PeResources::parse(b"MZ".to_vec()).is_err());
        assert!(PeResources::parse(vec![0; 64]).is_err());
    }
}
