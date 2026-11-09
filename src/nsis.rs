//! Reading the files out of a Nullsoft (NSIS 3) installer, which is how the OPU community update is distributed
//! (`OPUInstaller-1.4.1.exe`, NSIS 3.06.1, ANSI, one solid LZMA stream). Only what that needs is supported: NSIS 3
//! installers, ANSI or Unicode, compressed with LZMA (solid or per file). zlib and bzip2 installers are refused.
//!
//! The layout follows NSIS's `fileform.h` (and 7-Zip's NSIS handler, which documents the same structures): after the
//! loader stub, at a 512-byte boundary, a first header (`flags`, `0xDEADBEEF`, `"NullsoftInst"`, the header's size
//! and the size of everything after it), then the compressed header and file data. The header holds the install
//! script as fixed-size entries (an opcode and six parameters) and a string table. Files have no names of their
//! own: the script's `EW_EXTRACTFILE` entries name them, relative to the output folder the last `EW_CREATEDIR`
//! (`SetOutPath`) chose. [`Nsis::files`] walks the script in order to rebuild those paths; it doesn't follow the
//! script's jumps, so files the installer extracts only on some branches are all listed.

use std::io::Read;

use crate::Error;

const SIGNATURE: &[u8; 16] = b"\xEF\xBE\xAD\xDENullsoftInst";
/// Opcodes that don't depend on build options (NSIS 3 `fileform.h`).
const EW_CREATEDIR: i32 = 11;
const EW_EXTRACTFILE: i32 = 20;
const EW_ASSIGNVAR: i32 = 25;
/// Built-in variables after `$0`-`$9` and `$R0`-`$R9`, in order (`$CMDLINE` is 20).
const VARS: [&str; 12] =
    ["CMDLINE", "INSTDIR", "OUTDIR", "EXEDIR", "LANGUAGE", "TEMP", "PLUGINSDIR", "EXEPATH", "EXEFILE", "HWNDPARENT", "_CLICK", "_OUTDIR"];

/// A file the script extracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NsisFile {
    /// Where it goes, with variables in `$NAME` form and `\` separators: `$INSTDIR\OPU\README.md`,
    /// `$PLUGINSDIR\Helper.dll`. Variables the walk can't follow (user variables) stay as `$v63` etc.
    pub path: String,
    /// Offset of the file's block in the data section.
    offset: u32,
}

enum Data {
    /// The whole decompressed solid stream (header block first).
    Solid(Vec<u8>),
    /// The raw data section; each block is compressed on its own.
    PerFile(Vec<u8>),
}

pub struct Nsis {
    data: Data,
    /// Start of the file data in `data` (solid: after the header block).
    data_start: usize,
    files: Vec<NsisFile>,
}

impl Nsis {
    /// Parses an installer held in memory.
    pub fn parse(exe: &[u8]) -> Result<Nsis, Error> {
        let first = (0..exe.len().saturating_sub(28))
            .step_by(512)
            .find(|&o| &exe[o + 4..o + 20] == SIGNATURE)
            .ok_or_else(|| Error::Invalid("not an NSIS installer (no NullsoftInst header)".into()))?;
        let u32_at = |o: usize| u32::from_le_bytes(exe[o..o + 4].try_into().unwrap());
        let header_len = u32_at(first + 20) as usize;
        let total = u32_at(first + 24) as usize;
        let body = exe.get(first + 28..first + total).ok_or(Error::Truncated { offset: first + 28, wanted: total })?;
        if body.len() < 9 {
            return Err(Error::Truncated { offset: first + 28, wanted: 9 });
        }
        // Solid LZMA starts straight with the 5-byte properties (a properties byte, then a dictionary size); per-file
        // blocks start with a u32 size.
        let solid = body[0] < 225 && is_lzma_dict(&body[1..5]);
        let (data, header, data_start) = if solid {
            let all = lzma(body)?;
            let len = all.get(..4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize).unwrap_or(0);
            if len != header_len || all.len() < 4 + len {
                return Err(Error::Invalid("NSIS header size doesn't match (not a solid LZMA installer?)".into()));
            }
            let header = all[4..4 + len].to_vec();
            (Data::Solid(all), header, 4 + len)
        } else {
            let (header, used) = block(body, 0)?;
            if header.len() != header_len {
                return Err(Error::Unsupported("this NSIS installer isn't LZMA-compressed".into()));
            }
            (Data::PerFile(body.to_vec()), header, used)
        };
        let files = walk(&header)?;
        Ok(Nsis { data, data_start, files })
    }

    /// Every file the script extracts, in script order (the same file may be listed more than once).
    pub fn files(&self) -> &[NsisFile] {
        &self.files
    }

    /// The contents of a file.
    pub fn read(&self, f: &NsisFile) -> Result<Vec<u8>, Error> {
        let start = self.data_start + f.offset as usize;
        match &self.data {
            Data::Solid(all) => {
                let len = all.get(start..start + 4).ok_or(Error::Truncated { offset: start, wanted: 4 })?;
                let len = u32::from_le_bytes(len.try_into().unwrap()) as usize;
                Ok(all.get(start + 4..start + 4 + len).ok_or(Error::Truncated { offset: start + 4, wanted: len })?.to_vec())
            }
            Data::PerFile(raw) => Ok(block(raw, start)?.0),
        }
    }
}

/// A plausible LZMA dictionary size (a power of two, or 2^n + 2^(n-1)), from 4 KiB to 1 GiB.
fn is_lzma_dict(b: &[u8]) -> bool {
    let d = u32::from_le_bytes(b.try_into().unwrap());
    (12..=30).any(|n| d == 1 << n || d == (1 << n) | (1 << (n - 1)))
}

/// One per-file block at `pos`: a u32 size whose top bit means LZMA-compressed. Returns the data and the offset just
/// past the block.
fn block(raw: &[u8], pos: usize) -> Result<(Vec<u8>, usize), Error> {
    let head = raw.get(pos..pos + 4).ok_or(Error::Truncated { offset: pos, wanted: 4 })?;
    let v = u32::from_le_bytes(head.try_into().unwrap());
    let len = (v & 0x7FFF_FFFF) as usize;
    let body = raw.get(pos + 4..pos + 4 + len).ok_or(Error::Truncated { offset: pos + 4, wanted: len })?;
    let data = if v & 0x8000_0000 != 0 { lzma(body)? } else { body.to_vec() };
    Ok((data, pos + 4 + len))
}

/// Decompresses NSIS's LZMA: the properties byte and a u32 dictionary size, then the raw stream, which just ends.
fn lzma(stream: &[u8]) -> Result<Vec<u8>, Error> {
    let props = stream.get(..5).ok_or(Error::Truncated { offset: 0, wanted: 5 })?;
    if props[0] >= 225 {
        return Err(Error::Unsupported("this NSIS installer isn't LZMA-compressed (zlib and bzip2 aren't supported)".into()));
    }
    let dict = u32::from_le_bytes(props[1..5].try_into().unwrap());
    let mut r = lzma_rust2::LzmaReader::new_with_props(&stream[5..], u64::MAX, props[0], dict, None)
        .map_err(|e| Error::Invalid(format!("bad LZMA stream in the NSIS installer: {e}")))?;
    // The stream has no end marker and its size isn't stored: it ends where the input does. Deferring the error
    // that brings keeps the bytes decoded with it.
    r.set_defer_errors(true);
    let mut out = Vec::new();
    match r.read_to_end(&mut out) {
        Ok(_) => Ok(out),
        // NSIS streams carry no end marker; running out of input after the last byte is the normal end.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !out.is_empty() => Ok(out),
        Err(e) => Err(Error::Invalid(format!("bad LZMA stream in the NSIS installer: {e}"))),
    }
}

/// The header's string table and how to decode it.
struct Strings<'a> {
    table: &'a [u8],
    unicode: bool,
}

impl Strings<'_> {
    /// The string at `index` (a byte offset, or a UTF-16 unit offset for Unicode installers), with variables written
    /// as `$NAME` and language strings and shell folders as `$(lang N)`/`$(shell N)`.
    fn get(&self, index: i32) -> Result<String, Error> {
        let bad = || Error::Invalid(format!("NSIS string {index} is out of range"));
        let mut out = String::new();
        if self.unicode {
            let mut p = usize::try_from(index).map_err(|_| bad())? * 2;
            let unit = |p: usize| self.table.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(bad);
            loop {
                let c = unit(p)?;
                p += 2;
                match c {
                    0 => break,
                    0xE000 => {
                        out.extend(char::from_u32(unit(p)? as u32));
                        p += 2;
                    }
                    0xE001..=0xE003 => {
                        let n = unit(p)? & 0x7FFF;
                        p += 2;
                        push_code(&mut out, c - 0xE000, n);
                    }
                    c => out.extend(char::from_u32(c as u32)),
                }
            }
        } else {
            let mut p = usize::try_from(index).map_err(|_| bad())?;
            let byte = |p: usize| self.table.get(p).copied().ok_or_else(bad);
            loop {
                let c = byte(p)?;
                p += 1;
                match c {
                    0 => break,
                    // NSIS 3 ANSI codes: 1 language string, 2 shell folder, 3 variable, 4 skip (next byte literal).
                    4 => {
                        out.push(byte(p)? as char);
                        p += 1;
                    }
                    1..=3 => {
                        let n = (byte(p)? as u16 & 0x7F) | ((byte(p + 1)? as u16 & 0x7F) << 7);
                        p += 2;
                        push_code(&mut out, [0, 3, 2, 1][c as usize], n);
                    }
                    // ANSI strings are in the installer's code page; OPU's are ASCII.
                    c => out.push(c as char),
                }
            }
        }
        Ok(out)
    }
}

/// Appends a code: `kind` 1 variable, 2 shell folder, 3 language string (the Unicode numbering).
fn push_code(out: &mut String, kind: u16, n: u16) {
    match kind {
        1 => out.push_str(&var_name(n)),
        2 => out.push_str(&format!("$(shell {n})")),
        _ => out.push_str(&format!("$(lang {n})")),
    }
}

fn var_name(n: u16) -> String {
    match n {
        0..=9 => format!("${n}"),
        10..=19 => format!("$R{}", n - 10),
        20..=31 => format!("${}", VARS[n as usize - 20]),
        _ => format!("$v{n}"),
    }
}

/// Walks the script in order, following `$OUTDIR` and `$_OUTDIR`, and lists the extracted files.
fn walk(h: &[u8]) -> Result<Vec<NsisFile>, Error> {
    let u32_at = |o: usize| h.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated { offset: o, wanted: 4 });
    // flags, then 8 blocks of (offset, count): pages, sections, entries, strings, language tables, ...
    let entries_off = u32_at(4 + 2 * 8)? as usize;
    let entries_n = u32_at(4 + 2 * 8 + 4)? as usize;
    let strings_off = u32_at(4 + 3 * 8)? as usize;
    let langs_off = u32_at(4 + 4 * 8)? as usize;
    let table = h.get(strings_off..langs_off.max(strings_off)).ok_or(Error::Truncated { offset: strings_off, wanted: 0 })?;
    // A Unicode string table starts with an empty UTF-16 string.
    let strings = Strings { table, unicode: table.starts_with(&[0, 0]) };
    let entries = h.get(entries_off..entries_off + entries_n * 28).ok_or(Error::Truncated { offset: entries_off, wanted: entries_n * 28 })?;

    let mut outdir = String::from("$INSTDIR");
    let mut saved_outdir = String::from("$INSTDIR");
    let mut files = Vec::new();
    for e in entries.as_chunks::<28>().0 {
        let p: [i32; 7] = std::array::from_fn(|i| i32::from_le_bytes(e[i * 4..i * 4 + 4].try_into().unwrap()));
        let resolve = |s: String| s.replace("$_OUTDIR", &saved_outdir).replace("$OUTDIR", &outdir);
        match p[0] {
            EW_CREATEDIR if p[2] != 0 => outdir = resolve(strings.get(p[1])?),
            EW_ASSIGNVAR if p[1] == 31 => saved_outdir = resolve(strings.get(p[2])?),
            EW_EXTRACTFILE => {
                let name = resolve(strings.get(p[2])?);
                let path = if name.starts_with('$') || name.contains(":\\") { name } else { format!("{outdir}\\{name}") };
                files.push(NsisFile { path, offset: p[3] as u32 });
            }
            _ => {}
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_strings_decode_codes() {
        // "$INSTDIR\OPU", a skipped literal 3, and a user variable.
        let table = b"\0\x03\x95\x80\\OPU\0a\x04\x03b\0\x03\xBF\x80\0";
        let s = Strings { table, unicode: false };
        assert_eq!(s.get(1).unwrap(), "$INSTDIR\\OPU");
        assert_eq!(s.get(9).unwrap(), "a\x03b");
        assert_eq!(s.get(14).unwrap(), "$v63");
        assert!(s.get(100).is_err());
    }

    #[test]
    fn unicode_strings_decode_codes() {
        let units: [u16; 7] = [0, 0xE001, 21 | 0x8000, b'\\' as u16, b'x' as u16, 0, 0];
        let table: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
        let s = Strings { table: &table, unicode: true };
        assert_eq!(s.get(1).unwrap(), "$INSTDIR\\x");
    }

    /// A tiny installer: a header with three entries and four strings, then two files, in one solid stream.
    fn fake_installer(compress: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
        let strings = b"\0\x03\x95\x80\\OPU\0a.txt\0\x03\x9A\x80\\p.dll\0";
        let entry = |p: [i32; 7]| p.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
        let mut entries = Vec::new();
        entries.extend(entry([EW_EXTRACTFILE, 0, 15, 9, 0, 0, 0])); // $PLUGINSDIR\p.dll
        entries.extend(entry([EW_CREATEDIR, 1, 1, 0, 0, 0, 0])); // SetOutPath $INSTDIR\OPU
        entries.extend(entry([EW_EXTRACTFILE, 0, 9, 0, 0, 0, 0])); // a.txt
        let mut header = vec![0u8; 4 + 8 * 8];
        let entries_off = header.len() as u32;
        let strings_off = entries_off + entries.len() as u32;
        let langs_off = strings_off + strings.len() as u32;
        header[4 + 16..4 + 20].copy_from_slice(&entries_off.to_le_bytes());
        header[4 + 20..4 + 24].copy_from_slice(&3u32.to_le_bytes());
        header[4 + 24..4 + 28].copy_from_slice(&strings_off.to_le_bytes());
        header[4 + 32..4 + 36].copy_from_slice(&langs_off.to_le_bytes());
        header.extend(entries);
        header.extend(strings);
        let mut stream = (header.len() as u32).to_le_bytes().to_vec();
        stream.extend(&header);
        for f in [&b"hello"[..], &b"dll"[..]] {
            stream.extend((f.len() as u32).to_le_bytes());
            stream.extend(f);
        }
        let body = compress(&stream);
        let mut exe = vec![0u8; 1024];
        exe.extend(0u32.to_le_bytes());
        exe.extend(SIGNATURE);
        exe.extend((header.len() as u32).to_le_bytes());
        exe.extend(((28 + body.len()) as u32).to_le_bytes());
        exe.extend(body);
        exe
    }

    #[test]
    fn lists_and_reads_files_of_a_solid_installer() {
        let exe = fake_installer(|stream| {
            let mut opts = lzma_rust2::LzmaOptions::with_preset(1);
            opts.dict_size = 1 << 16;
            let mut w = lzma_rust2::LzmaWriter::new_no_header(Vec::new(), &opts, false).unwrap();
            std::io::Write::write_all(&mut w, stream).unwrap();
            let raw = w.finish().unwrap();
            let mut out = vec![opts.get_props()];
            out.extend(opts.dict_size.to_le_bytes());
            out.extend(raw);
            out
        });
        let n = Nsis::parse(&exe).unwrap();
        let paths: Vec<&str> = n.files().iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["$PLUGINSDIR\\p.dll", "$INSTDIR\\OPU\\a.txt"]);
        assert_eq!(n.read(&n.files()[1]).unwrap(), b"hello");
        assert_eq!(n.read(&n.files()[0]).unwrap(), b"dll");
        assert!(Nsis::parse(&exe[..exe.len() - 40]).is_err());
        assert!(Nsis::parse(b"MZ not an installer").is_err());
    }
}
