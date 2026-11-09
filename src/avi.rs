//! AVI (RIFF `AVI `) container: stream headers and the chunks of each stream, in file order.
//!
//! Enough for the game's cutscenes (one Cinepak video stream, one MS ADPCM audio stream). The
//! `movi` list is scanned directly (recursing into `rec ` lists), so a missing or broken `idx1`
//! index doesn't matter. OpenDML (`AVIX`, files over 1 GiB) is not supported.

use crate::Error;
use crate::reader::Reader;
use crate::sound::WaveFormat;

#[derive(Debug, Clone)]
pub enum StreamFormat {
    /// BITMAPINFOHEADER fields.
    Video { width: u32, height: u32, bit_count: u16, compression: [u8; 4] },
    /// WAVEFORMATEX plus its extra bytes (`cbSize` of them, e.g. the MS ADPCM coefficients).
    Audio { format: WaveFormat, extra: Vec<u8> },
    Other,
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub kind: [u8; 4],
    pub handler: [u8; 4],
    pub scale: u32,
    pub rate: u32,
    pub length: u32,
    pub format: StreamFormat,
    /// (file offset, length) of each data chunk of this stream, in file order.
    pub chunks: Vec<(usize, usize)>,
}

impl Stream {
    /// Frames (or blocks) per second.
    pub fn fps(&self) -> f64 {
        self.rate as f64 / self.scale.max(1) as f64
    }
}

pub struct Avi {
    data: Vec<u8>,
    pub micros_per_frame: u32,
    pub total_frames: u32,
    pub width: u32,
    pub height: u32,
    pub streams: Vec<Stream>,
}

impl Avi {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let mut r = Reader::new(&data);
        r.expect_tag(b"RIFF")?;
        r.u32()?;
        r.expect_tag(b"AVI ")?;
        let mut avi = Avi { data: Vec::new(), micros_per_frame: 0, total_frames: 0, width: 0, height: 0, streams: Vec::new() };
        walk(&data, 12, data.len(), &mut avi, 0)?;
        if avi.streams.is_empty() {
            return Err(Error::Invalid("AVI has no streams".into()));
        }
        avi.data = data;
        Ok(avi)
    }

    pub fn chunk(&self, (offset, len): (usize, usize)) -> &[u8] {
        &self.data[offset..offset + len]
    }

    pub fn video_stream(&self) -> Option<usize> {
        self.streams.iter().position(|s| &s.kind == b"vids")
    }

    pub fn audio_stream(&self) -> Option<usize> {
        self.streams.iter().position(|s| &s.kind == b"auds")
    }
}

fn walk(data: &[u8], mut pos: usize, end: usize, avi: &mut Avi, depth: u32) -> Result<(), Error> {
    if depth > 8 {
        return Err(Error::Invalid("AVI lists nested too deeply".into()));
    }
    while pos + 8 <= end {
        let mut r = Reader::at(data, pos);
        let tag = r.tag()?;
        let len = r.u32()? as usize;
        let body = pos + 8;
        // Truncated files: clamp the last chunk rather than failing.
        let body_end = body.saturating_add(len).min(end);
        match &tag {
            b"LIST" => {
                let list_kind = Reader::at(data, body).tag()?;
                if &list_kind == b"strl" {
                    avi.streams.push(Stream {
                        kind: [0; 4],
                        handler: [0; 4],
                        scale: 1,
                        rate: 0,
                        length: 0,
                        format: StreamFormat::Other,
                        chunks: Vec::new(),
                    });
                }
                walk(data, body + 4, body_end, avi, depth + 1)?;
            }
            b"avih" => {
                let mut h = Reader::at(data, body);
                avi.micros_per_frame = h.u32()?;
                h.skip(12)?;
                avi.total_frames = h.u32()?;
                h.skip(12)?;
                avi.width = h.u32()?;
                avi.height = h.u32()?;
            }
            b"strh" => {
                let s = avi.streams.last_mut().ok_or_else(|| Error::Invalid("strh outside strl".into()))?;
                let mut h = Reader::at(data, body);
                s.kind = h.tag()?;
                s.handler = h.tag()?;
                h.skip(12)?; // flags, priority, language, initial frames
                s.scale = h.u32()?;
                s.rate = h.u32()?;
                h.u32()?; // start
                s.length = h.u32()?;
            }
            b"strf" => {
                let s = avi.streams.last_mut().ok_or_else(|| Error::Invalid("strf outside strl".into()))?;
                let mut h = Reader::at(data, body);
                s.format = match &s.kind {
                    b"vids" => {
                        h.u32()?;
                        let width = h.i32()?.unsigned_abs();
                        let height = h.i32()?.unsigned_abs();
                        h.u16()?;
                        let bit_count = h.u16()?;
                        let compression = h.tag()?;
                        StreamFormat::Video { width, height, bit_count, compression }
                    }
                    b"auds" => {
                        let format = WaveFormat {
                            format_tag: h.u16()?,
                            channels: h.u16()?,
                            samples_per_sec: h.u32()?,
                            avg_bytes_per_sec: h.u32()?,
                            block_align: h.u16()?,
                            bits_per_sample: h.u16()?,
                        };
                        let extra = if len > 16 {
                            let cb = h.u16()? as usize;
                            h.bytes(cb.min(len - 18))?.to_vec()
                        } else {
                            Vec::new()
                        };
                        StreamFormat::Audio { format, extra }
                    }
                    _ => StreamFormat::Other,
                };
            }
            _ => {
                // Stream data: "NNdc", "NNdb", "NNwb", "NNpc" with NN the stream number.
                if tag[0].is_ascii_digit() && tag[1].is_ascii_digit() {
                    let n = ((tag[0] - b'0') * 10 + (tag[1] - b'0')) as usize;
                    if &tag[2..] != b"pc"
                        && let Some(s) = avi.streams.get_mut(n)
                    {
                        s.chunks.push((body, body_end - body));
                    }
                }
            }
        }
        pos = body + len + (len & 1);
    }
    Ok(())
}
