//! UI resources and cutscene media against a real game install. Skipped (passing) without one.
//! The media hashes were taken from output that matched ffmpeg's decoders bit for bit.

use op2_formats::adpcm::MsAdpcm;
use op2_formats::avi::{Avi, StreamFormat};
use op2_formats::cinepak::Cinepak;
use op2_formats::pe_res::{self, AniCursor, Dialog, Dib, PeResources, ResId};
use op2_formats::testdata::game_dir;

fn fnv(data: impl IntoIterator<Item = u8>) -> u64 {
    data.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

fn resources(dir: &std::path::Path, name: &str) -> PeResources {
    PeResources::parse(std::fs::read(dir.join(name)).unwrap()).unwrap()
}

#[test]
fn resource_dlls_decode() {
    let Some(dir) = game_dir() else { return };
    let out2res = resources(&dir, "out2res.dll");
    let shres = resources(&dir, "op2shres.dll");
    let count = |r: &PeResources, t: u16| r.of_type(&ResId::Id(t)).count();
    assert_eq!((count(&out2res, pe_res::RT_BITMAP), count(&out2res, pe_res::RT_DIALOG)), (36, 19));
    assert_eq!((count(&shres, pe_res::RT_BITMAP), count(&shres, pe_res::RT_DIALOG)), (67, 16));

    for res in [&out2res, &shres] {
        for r in res.of_type(&ResId::Id(pe_res::RT_BITMAP)) {
            let dib = Dib::parse(res.bytes(r)).unwrap_or_else(|e| panic!("bitmap {}: {e}", r.name));
            assert_eq!(dib.rgba.len(), (dib.width * dib.height * 4) as usize);
        }
        for r in res.of_type(&ResId::Id(pe_res::RT_DIALOG)) {
            let d = Dialog::parse(res.bytes(r)).unwrap_or_else(|e| panic!("dialog {}: {e}", r.name));
            assert_eq!(d.font.as_ref().map(|f| f.1.as_str()), Some("MS Sans Serif"));
        }
    }
    // The command pane divider; the shell's main-menu background is RLE8.
    let divider = out2res.bitmap(&ResId::Id(184)).unwrap();
    assert_eq!((divider.width, divider.height, divider.bit_count), (8, 378, 8));
    let menu = shres.bitmap(&ResId::Id(190)).unwrap();
    assert_eq!((menu.width, menu.height, menu.compression), (640, 480, 1));

    let strings = shres.strings().unwrap();
    assert!(strings.iter().any(|(id, s)| *id == 701 && s.contains("MS Sans Serif")), "{strings:?}");
    let main_menu = Dialog::parse(shres.bytes(shres.find(&ResId::Id(pe_res::RT_DIALOG), &ResId::Id(137)).unwrap())).unwrap();
    assert_eq!(main_menu.title, "MAIN MENU");

    let cursor = out2res.cursor_or_icon(pe_res::RT_GROUP_CURSOR, &ResId::Id(109)).unwrap();
    assert_eq!((cursor.image.width, cursor.image.height), (32, 32));
}

#[test]
fn ani_cursors_decode() {
    let Some(dir) = game_dir() else { return };
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ani")) {
            let ani = AniCursor::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            assert!(ani.frames.iter().all(|f| f.image.width == 32 && f.image.height == 32));
            n += 1;
        }
    }
    assert_eq!(n, 17);
}

#[test]
fn cutscene_decodes_like_reference() {
    let Some(dir) = game_dir() else { return };
    let avi = Avi::parse(std::fs::read(dir.join("CEP.avi")).unwrap()).unwrap();

    let v = &avi.streams[avi.video_stream().unwrap()];
    let StreamFormat::Video { width, height, .. } = v.format else { panic!("no video format") };
    let mut dec = Cinepak::new(width, height);
    let mut frames_hash = 0u64;
    for &c in &v.chunks {
        dec.decode(avi.chunk(c)).unwrap();
        frames_hash ^= fnv(dec.rgb.iter().copied());
        frames_hash = frames_hash.rotate_left(1);
    }
    assert_eq!(v.chunks.len(), 207);

    let a = &avi.streams[avi.audio_stream().unwrap()];
    let StreamFormat::Audio { format, extra } = &a.format else { panic!("no audio format") };
    let codec = MsAdpcm::new(format.channels, format.block_align, extra).unwrap();
    let mut pcm = Vec::new();
    for &c in &a.chunks {
        codec.decode(avi.chunk(c), &mut pcm).unwrap();
    }
    let audio_hash = fnv(pcm.iter().flat_map(|s| s.to_le_bytes()));
    println!("frames {frames_hash:#018x} audio {audio_hash:#018x} samples {}", pcm.len());
    assert_eq!(pcm.len(), 605_176);
    assert_eq!(frames_hash, FRAMES_HASH);
    assert_eq!(audio_hash, AUDIO_HASH);
}

const FRAMES_HASH: u64 = 0x2736_ce4c_c75f_fe83;
const AUDIO_HASH: u64 = 0x8d2f_fa92_07dd_ab6c;
