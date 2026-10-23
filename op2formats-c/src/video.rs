//! The cutscenes: AVI files with Cinepak video and MS ADPCM audio.

use crate::*;
use op2_formats::adpcm::MsAdpcm;
use op2_formats::avi::{Avi, StreamFormat};
use op2_formats::cinepak::{self, Cinepak};
use std::ffi::c_char;
use std::sync::Mutex;

/// An AVI file with a decoder for its video. Frames can be asked for in any order: the decoder goes back to the
/// nearest keyframe when it has to. One thread at a time may decode from a handle.
pub struct Op2fVideo(pub(crate) Video);

pub(crate) struct Video {
    avi: Avi,
    /// The video stream, when it is Cinepak.
    video: Option<usize>,
    /// The decoder and the frame it holds (`None` before the first decode).
    state: Mutex<(Cinepak, Option<usize>)>,
}

/// An AVI's overall numbers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fVideoInfo {
    pub width: u32,
    pub height: u32,
    /// Video frames (0 without a Cinepak video stream).
    pub frame_count: u32,
    /// Frames per second as rate / scale.
    pub rate: u32,
    pub scale: u32,
    pub micros_per_frame: u32,
    pub has_audio: bool,
    /// The audio stream's format (format_tag 2 is MS ADPCM; `op2f_video_audio` decodes it to 16-bit PCM).
    pub audio: Op2fWaveFormat,
}

/// One stream of an AVI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fStreamInfo {
    /// "vids", "auds", ...
    pub kind: [u8; 4],
    /// The codec's FourCC, e.g. "cvid".
    pub handler: [u8; 4],
    pub scale: u32,
    pub rate: u32,
    pub length: u32,
    pub chunk_count: usize,
}

fn new_video(avi: Avi) -> Op2fVideo {
    let video = avi.video_stream().filter(|&i| matches!(avi.streams[i].format, StreamFormat::Video { compression, .. } if &compression == b"cvid"));
    let decoder = Cinepak::new(avi.width, avi.height);
    Op2fVideo(Video { avi, video, state: Mutex::new((decoder, None)) })
}

/// Reads a cutscene from the game by name (e.g. "MEI.AVI").
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fVideo) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (data, _) = game.files.read(unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, new_video(Avi::parse(data).map_err(lib_err)?)) }
    })
}

/// Parses an AVI from memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_parse(data: *const u8, len: usize, out: *mut *mut Op2fVideo) -> Op2fStatus {
    guard(|| {
        let avi = Avi::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, new_video(avi)) }
    })
}

/// # Safety
/// `video` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_free(video: *mut Op2fVideo) {
    unsafe { free(video) }
}

/// # Safety
/// `video` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_info(video: *const Op2fVideo, out: *mut Op2fVideoInfo) -> Op2fStatus {
    guard(|| {
        let v = &unsafe { obj(video, "video") }?.0;
        let vs = v.video.map(|i| &v.avi.streams[i]);
        let audio = v.avi.audio_stream().and_then(|i| match &v.avi.streams[i].format {
            StreamFormat::Audio { format, .. } => Some(sound::wave_format(format)),
            _ => None,
        });
        let info = Op2fVideoInfo {
            width: v.avi.width,
            height: v.avi.height,
            frame_count: vs.map_or(0, |s| s.chunks.len() as u32),
            rate: vs.map_or(0, |s| s.rate),
            scale: vs.map_or(1, |s| s.scale),
            micros_per_frame: v.avi.micros_per_frame,
            has_audio: audio.is_some(),
            audio: audio.unwrap_or(Op2fWaveFormat {
                format_tag: 0,
                channels: 0,
                samples_per_sec: 0,
                avg_bytes_per_sec: 0,
                block_align: 0,
                bits_per_sample: 0,
            }),
        };
        unsafe { put_value(out, info) }
    })
}

fn video_chunks(v: &Video) -> Result<&[(usize, usize)], Op2fStatus> {
    let i = v.video.ok_or_else(|| fail(Op2fStatus::Unsupported, "the AVI has no Cinepak video stream"))?;
    Ok(&v.avi.streams[i].chunks)
}

/// Whether video frame `frame` decodes on its own (a keyframe, where playback can start).
///
/// # Safety
/// `video` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_is_keyframe(video: *const Op2fVideo, frame: usize, out: *mut bool) -> Op2fStatus {
    guard(|| {
        let v = &unsafe { obj(video, "video") }?.0;
        let chunk = *index(video_chunks(v)?, frame, "frame")?;
        unsafe { put_value(out, cinepak::is_keyframe(v.avi.chunk(chunk), v.avi.height)) }
    })
}

/// Decodes video frame `frame` as RGBA (alpha 255): width * height * 4 bytes into `out`, which holds `cap`. Playing
/// frames in order costs one decode each; jumping decodes from the nearest keyframe before it.
///
/// # Safety
/// `video` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_frame(video: *const Op2fVideo, frame: usize, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let v = &unsafe { obj(video, "video") }?.0;
        let chunks = video_chunks(v)?;
        index(chunks, frame, "frame")?;
        let need = v.avi.width as usize * v.avi.height as usize * 4;
        if cap < need {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("a frame needs {need} bytes, buffer holds {cap}")));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        let mut state = v.state.lock().unwrap_or_else(|e| e.into_inner());
        let (decoder, held) = &mut *state;
        // Go on from the frame held, unless it's past `frame` or a keyframe lies between them.
        let key = (0..=frame).rev().find(|&f| cinepak::is_keyframe(v.avi.chunk(chunks[f]), v.avi.height)).unwrap_or(0);
        let start = match *held {
            Some(h) if h <= frame && h + 1 >= key => h + 1,
            _ => {
                *decoder = Cinepak::new(v.avi.width, v.avi.height);
                key
            }
        };
        for (f, &chunk) in chunks.iter().enumerate().take(frame + 1).skip(start) {
            *held = None;
            decoder.decode(v.avi.chunk(chunk)).map_err(|e| lib_err(op2_formats::Error::Invalid(format!("frame {f}: {e}"))))?;
            *held = Some(f);
        }
        let out = unsafe { std::slice::from_raw_parts_mut(out, need) };
        for (dst, src) in out.as_chunks_mut::<4>().0.iter_mut().zip(decoder.rgb.as_chunks::<3>().0) {
            dst.copy_from_slice(&[src[0], src[1], src[2], 255]);
        }
        Ok(())
    })
}

/// The whole audio track decoded to interleaved signed 16-bit PCM (MS ADPCM is decoded; 16-bit PCM is copied), at
/// the rate and channels `op2f_video_info` gives.
///
/// # Safety
/// `video` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_audio(video: *const Op2fVideo, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let v = &unsafe { obj(video, "video") }?.0;
        let i = v.avi.audio_stream().ok_or_else(|| fail(Op2fStatus::NotFound, "the AVI has no audio stream"))?;
        let s = &v.avi.streams[i];
        let StreamFormat::Audio { format, extra } = &s.format else {
            return Err(fail(Op2fStatus::InvalidData, "audio stream without a WAVEFORMATEX"));
        };
        let bytes = match (format.format_tag, format.bits_per_sample) {
            (2, _) => {
                let codec = MsAdpcm::new(format.channels, format.block_align, extra).map_err(lib_err)?;
                let mut pcm = Vec::new();
                for &c in &s.chunks {
                    codec.decode(v.avi.chunk(c), &mut pcm).map_err(lib_err)?;
                }
                pcm.iter().flat_map(|s| s.to_le_bytes()).collect()
            }
            (1, 16) => s.chunks.iter().flat_map(|&c| v.avi.chunk(c).iter().copied()).collect(),
            (tag, bits) => return Err(fail(Op2fStatus::Unsupported, format!("audio format {tag} with {bits} bits"))),
        };
        unsafe { put(out, Op2fBytes(bytes)) }
    })
}

/// Number of streams.
///
/// # Safety
/// `video` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_stream_count(video: *const Op2fVideo) -> usize {
    unsafe { video.as_ref() }.map_or(0, |v| v.0.avi.streams.len())
}

/// # Safety
/// `video` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_stream_info(video: *const Op2fVideo, stream: usize, out: *mut Op2fStreamInfo) -> Op2fStatus {
    guard(|| {
        let s = index(&unsafe { obj(video, "video") }?.0.avi.streams, stream, "stream")?;
        let info = Op2fStreamInfo { kind: s.kind, handler: s.handler, scale: s.scale, rate: s.rate, length: s.length, chunk_count: s.chunks.len() };
        unsafe { put_value(out, info) }
    })
}

/// The raw bytes of data chunk `chunk` of a stream (an encoded video frame, or a block of audio).
///
/// # Safety
/// `video` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_video_chunk(video: *const Op2fVideo, stream: usize, chunk: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let avi = &unsafe { obj(video, "video") }?.0.avi;
        let s = index(&avi.streams, stream, "stream")?;
        let c = *index(&s.chunks, chunk, "chunk")?;
        unsafe { put(out, Op2fBytes(avi.chunk(c).to_vec())) }
    })
}
