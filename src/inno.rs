//! Extracting the game folder from GOG's offline installer (`setup_outpost_2_*.exe`, plus any `-N.bin` parts next to
//! it), an Inno Setup installer, so a player who only kept the download can play without running it under Wine.
//! Only files the player points at are read; nothing is downloaded (docs/legal.md rule 1).
//!
//! Supports the Unicode installers of Inno Setup 5.5.0 to 6.3 (GOG's Outpost 2 installer says "5.6.2", a GOG build
//! between 5.6.1 and 6.0). The layout follows Inno Setup's own `Struct.pas` and innoextract's notes on how it
//! changed between versions; everything read is untrusted and bounds-checked. Encrypted and bzip2-compressed
//! installers aren't supported: innoextract handles those (docs/game-data.md).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use crate::Error;

const fn ver(a: u32, b: u32, c: u32, d: u32) -> u32 {
    (a << 24) | (b << 16) | (c << 8) | d
}

/// `TSetupLdrOffsetTable.ID` for Inno Setup 5.1.5 and later, and a variant innoextract has seen.
const OFFSET_TABLE_IDS: [&[u8; 12]; 2] = [b"rDlPtS\xCD\xE6\xD7\x7B\x0B\x2A", b"nS5W7dT\x83\xAA\x1B\x0F\x6A"];
/// The RCDATA resource holding the offset table.
const OFFSET_TABLE_RESOURCE: u16 = 11111;
/// Prefix of every compressed file chunk.
const CHUNK_ID: &[u8; 4] = b"zlb\x1a";
/// `-N.bin` parts start with this and a u32 size.
const SLICE_IDS: [&[u8; 8]; 2] = [b"idska32\x1a", b"idska16\x1a"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compression {
    Stored,
    Zlib,
    Bzip2,
    Lzma1,
    Lzma2,
}

#[derive(Clone, Debug)]
struct FileEntry {
    /// Destination as written in the script, e.g. `{app}\maps.vol`.
    dest: String,
    languages: String,
    /// Index into the data entries, or `u32::MAX` for files not stored in the installer.
    location: u32,
}

#[derive(Clone, Debug)]
struct DataEntry {
    first_slice: u32,
    chunk_offset: u32,
    /// Offset of the file in the decompressed chunk.
    file_offset: u64,
    file_size: u64,
    chunk_size: u64,
    sha1: [u8; 20],
    /// x86 CALL/JMP addresses were made relative before compression (Inno's "CallInstructionOptimized").
    filtered: bool,
    compressed: bool,
    encrypted: bool,
}

/// An opened installer: its header parsed, nothing extracted yet.
#[derive(Debug)]
pub struct Installer {
    path: PathBuf,
    /// The installer's data version line, e.g. "Inno Setup Setup Data (5.6.2) (u)".
    pub version_text: String,
    /// The product it installs ("Outpost 2" for GOG's).
    pub app_name: String,
    compression: Compression,
    slices_per_disk: u32,
    /// Where the file data starts in the .exe, or 0 when it's in `-N.bin` parts.
    data_offset: u32,
    files: Vec<FileEntry>,
    data: Vec<DataEntry>,
}

/// What [`Installer::extract`] wrote.
#[derive(Debug, Default)]
pub struct Extracted {
    pub files: usize,
    pub bytes: u64,
}

impl Installer {
    /// Reads the installer's header. Errors say what's wrong in words a player can act on.
    pub fn open(path: &Path) -> Result<Installer, Error> {
        let mut f = File::open(path).map_err(|e| io_err(path, e))?;
        let len = f.metadata().map_err(|e| io_err(path, e))?.len();
        // The setup loader with its resources is a small PE image at the start; the payload follows it.
        let mut head = Vec::new();
        (&mut f).take(len.min(16 << 20)).read_to_end(&mut head).map_err(|e| io_err(path, e))?;
        let not_inno = || Error::Invalid(format!("{} is not an Inno Setup installer", path.display()));
        let res = crate::pe_res::PeResources::parse(head).map_err(|_| not_inno())?;
        let table = res
            .find(&crate::pe_res::ResId::Id(crate::pe_res::RT_RCDATA), &crate::pe_res::ResId::Id(OFFSET_TABLE_RESOURCE))
            .map(|r| res.bytes(r).to_vec())
            .ok_or_else(not_inno)?;
        let (header_offset, data_offset) = parse_offset_table(&table).ok_or_else(not_inno)?;

        f.seek(SeekFrom::Start(header_offset as u64)).map_err(|e| io_err(path, e))?;
        let mut id = [0u8; 64];
        f.read_exact(&mut id).map_err(|e| io_err(path, e))?;
        let version_text = String::from_utf8_lossy(&id).trim_end_matches('\0').to_string();
        let version = parse_version(&version_text).ok_or_else(|| {
            Error::Unsupported(format!("unrecognised Inno Setup data version {version_text:?}; try innoextract"))
        })?;
        let unicode = version >= ver(6, 3, 0, 0) || version_text.contains("(u)") || version_text.contains("(U)");
        if !unicode || version < ver(5, 5, 0, 0) || version >= ver(6, 4, 0, 0) {
            return Err(Error::Unsupported(format!(
                "{version_text:?} installers aren't supported (op2-rs reads Unicode Inno Setup 5.5 to 6.3, as GOG \
                 uses); extract it with innoextract instead"
            )));
        }

        let mut r = BufReader::new(f);
        let primary = read_block(&mut r).map_err(|e| e.context("reading the installer's header"))?;
        let secondary = read_block(&mut r).map_err(|e| e.context("reading the installer's file table"))?;
        let mut inst = parse_headers(&primary, &secondary, version)?;
        inst.path = path.to_path_buf();
        inst.version_text = version_text;
        inst.data_offset = data_offset;
        Ok(inst)
    }

    /// The installed files (relative paths below the game folder) and their sizes.
    pub fn game_files(&self) -> Vec<(PathBuf, u64)> {
        self.chosen().into_iter().map(|(rel, d)| (rel, self.data[d].file_size)).collect()
    }

    /// Total bytes [`extract`](Self::extract) writes.
    /// Bytes [`extract`](Self::extract) decompresses (identical files installed under several names count once).
    pub fn total_size(&self) -> u64 {
        let mut entries: Vec<usize> = self.chosen().into_iter().map(|(_, d)| d).collect();
        entries.sort_unstable();
        entries.dedup();
        entries.iter().map(|&d| self.data[d].file_size).sum()
    }

    /// One data entry per destination below `{app}`: when several entries write the same file (per-language
    /// variants), the English or language-neutral one.
    fn chosen(&self) -> Vec<(PathBuf, usize)> {
        let mut by_dest: HashMap<String, (PathBuf, usize, bool)> = HashMap::new();
        let mut order = Vec::new();
        for f in &self.files {
            let Some(rel) = app_relative(&f.dest) else { continue };
            if f.location as usize >= self.data.len() {
                continue;
            }
            let langs = f.languages.to_ascii_lowercase();
            let preferred = langs.is_empty() || langs.split([' ', ',']).any(|l| l == "english" || l == "en");
            let key = rel.to_string_lossy().to_lowercase();
            match by_dest.get_mut(&key) {
                Some(e) if !e.2 && preferred => *e = (rel, f.location as usize, preferred),
                Some(_) => {}
                None => {
                    order.push(key.clone());
                    by_dest.insert(key, (rel, f.location as usize, preferred));
                }
            }
        }
        order.into_iter().filter_map(|k| by_dest.remove(&k)).map(|(p, d, _)| (p, d)).collect()
    }

    /// Extracts the game folder into `out` (created if needed), checking every file's SHA-1 against the installer's.
    /// `progress` gets (bytes written, total) as it goes.
    pub fn extract(&self, out: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<Extracted, Error> {
        let chosen = self.chosen();
        let total = self.total_size();
        // Destinations per data entry (identical files are stored once and installed under several names).
        let mut dests: HashMap<usize, Vec<PathBuf>> = HashMap::new();
        for (rel, d) in &chosen {
            dests.entry(*d).or_default().push(out.join(rel));
        }
        // Chunks in file order, each with its files sorted by offset.
        let mut chunks: Vec<((u32, u32), Vec<usize>)> = Vec::new();
        for &d in dests.keys() {
            let e = &self.data[d];
            if e.encrypted {
                return Err(Error::Unsupported("this installer is password-protected; extract it with innoextract".into()));
            }
            let key = (e.first_slice, e.chunk_offset);
            match chunks.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(d),
                None => chunks.push((key, vec![d])),
            }
        }
        chunks.sort_by_key(|(k, _)| *k);
        for (_, v) in &mut chunks {
            v.sort_by_key(|&d| (self.data[d].file_offset, self.data[d].file_size));
        }

        let mut done = Extracted::default();
        let mut slices = SliceReader::new(self)?;
        for (_, entries) in &chunks {
            let first = &self.data[entries[0]];
            slices.seek(first.first_slice, first.chunk_offset)?;
            let mut id = [0u8; 4];
            slices.read_exact(&mut id).map_err(|e| self.io(e))?;
            if &id != CHUNK_ID {
                return Err(Error::Invalid(format!("{}: bad chunk header at {:#x}", self.path.display(), first.chunk_offset)));
            }
            let files = entries
                .iter()
                .map(|&d| PendingFile::new(&self.data[d], dests.remove(&d).unwrap_or_default()))
                .collect::<Vec<_>>();
            let needed = files.iter().map(|f| f.offset + f.size).max().unwrap_or(0);
            let mut sink = ChunkSink { pos: 0, files, next: 0, needed, written: done.bytes, total, progress, error: None };
            let compressed = (&mut slices).take(first.chunk_size);
            let result = if needed == 0 {
                Ok(()) // only empty files
            } else {
                decompress(if first.compressed { self.compression } else { Compression::Stored }, compressed, &mut sink, needed)
            };
            if let Some(e) = sink.error.take() {
                return Err(e);
            }
            sink.feed(&[])?;
            if sink.next < sink.files.len() {
                let name = sink.files[sink.next].outputs.first().map(|p| p.display().to_string()).unwrap_or_default();
                let why = result.err().map(|e| e.to_string()).unwrap_or_else(|| "data ended early".into());
                return Err(Error::Invalid(format!("{}: decompressing {name}: {why}", self.path.display())));
            }
            done.files += sink.files.iter().map(|f| f.outputs.len()).sum::<usize>();
            done.bytes = sink.written;
        }
        Ok(done)
    }

    fn io(&self, e: io::Error) -> Error {
        io_err(&self.path, e)
    }
}

/// `{app}\sub\file` as a safe relative path (`sub/file`); `None` for other destinations or anything that could
/// escape the output folder.
fn app_relative(dest: &str) -> Option<PathBuf> {
    let rest = dest.get(..6).filter(|p| p.eq_ignore_ascii_case("{app}\\")).map(|_| &dest[6..])?;
    let mut out = PathBuf::new();
    for part in rest.split(['\\', '/']).filter(|p| !p.is_empty()) {
        let comp = Path::new(part).components().next()?;
        if !matches!(comp, Component::Normal(_)) || part.contains(':') || part.contains('{') {
            return None;
        }
        out.push(part);
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// (setup data offset, file data offset) from `TSetupLdrOffsetTable`, checking its CRC.
fn parse_offset_table(t: &[u8]) -> Option<(u32, u32)> {
    if t.len() < 44 || !OFFSET_TABLE_IDS.iter().any(|id| &t[..12] == *id) {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes(t[o..o + 4].try_into().unwrap());
    if u32_at(12) != 1 || crc32fast::hash(&t[..40]) != u32_at(40) {
        return None;
    }
    // ID, Version, TotalSize, OffsetEXE, UncompressedSizeEXE, CRCEXE, Offset0, Offset1, TableCRC
    Some((u32_at(32), u32_at(36)))
}

/// "Inno Setup Setup Data (5.6.2) (u)" -> 5.6.2.0; four-part versions ("6.4.0.1") keep the fourth part.
fn parse_version(text: &str) -> Option<u32> {
    let inner = text.strip_prefix("Inno Setup Setup Data (")?;
    let nums = &inner[..inner.find(')')?];
    let parts: Vec<u32> = nums.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    if parts.len() < 3 || parts.len() > 4 || parts.iter().any(|&p| p > 255) {
        return None;
    }
    Some(ver(parts[0], parts[1], parts[2], parts.get(3).copied().unwrap_or(0)))
}

/// One compressed header block: a CRC-checked size and flag, then 4 KiB pieces each led by its CRC32, LZMA1 when
/// compressed.
fn read_block(r: &mut impl Read) -> Result<Vec<u8>, Error> {
    let mut h = [0u8; 9];
    r.read_exact(&mut h).map_err(|_| Error::Invalid("truncated header block".into()))?;
    let crc = u32::from_le_bytes(h[..4].try_into().unwrap());
    if crc32fast::hash(&h[4..9]) != crc {
        return Err(Error::Invalid("header block CRC mismatch (damaged or incomplete download?)".into()));
    }
    let stored = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
    if stored > 256 << 20 {
        return Err(Error::Invalid("implausible header block size".into()));
    }
    let mut raw = vec![0u8; stored];
    r.read_exact(&mut raw).map_err(|_| Error::Invalid("truncated header block".into()))?;
    let mut payload = Vec::with_capacity(stored);
    for piece in raw.chunks(4 + 4096) {
        if piece.len() < 5 {
            return Err(Error::Invalid("header block ends mid-piece".into()));
        }
        let (crc, data) = piece.split_at(4);
        if crc32fast::hash(data) != u32::from_le_bytes(crc.try_into().unwrap()) {
            return Err(Error::Invalid("header block CRC mismatch (damaged or incomplete download?)".into()));
        }
        payload.extend_from_slice(data);
    }
    if h[8] == 0 {
        return Ok(payload);
    }
    let bad = |e: &dyn std::fmt::Display| Error::Invalid(format!("decompressing a header block: {e}"));
    let (props, data) = payload.split_at_checked(5).ok_or_else(|| bad(&"too short"))?;
    let mut out = Vec::new();
    lzma1_reader(data, props.try_into().unwrap(), u64::MAX)?.read_to_end(&mut out).map_err(|e| bad(&e))?;
    Ok(out)
}

/// A raw LZMA1 stream as Inno Setup writes it: a properties byte and a u32 dictionary size, then the data.
/// `size` is the decompressed size, or `u64::MAX` when the stream just ends.
fn lzma1_reader<R: Read>(input: R, props: &[u8; 5], size: u64) -> Result<lzma_rust2::LzmaReader<R>, Error> {
    let dict = u32::from_le_bytes(props[1..5].try_into().unwrap());
    // As for LZMA2, a dictionary bigger than the data is never used.
    let dict = if size == u64::MAX { dict } else { dict.min(size.clamp(4096, u32::MAX as u64) as u32) };
    lzma_rust2::LzmaReader::new_with_props(input, size, props[0], dict, None)
        .map_err(|e| Error::Invalid(format!("bad LZMA stream: {e}")))
}

/// Bounds-checked reads over a decompressed header.
struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or(Error::Truncated { offset: self.pos, wanted: n })?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.bytes(n).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    /// A length-prefixed string's raw bytes.
    fn raw_string(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.bytes(n)
    }
    fn skip_strings(&mut self, n: usize) -> Result<(), Error> {
        (0..n).try_for_each(|_| self.raw_string().map(|_| ()))
    }
    /// A length-prefixed UTF-16LE string.
    fn wstring(&mut self) -> Result<String, Error> {
        let raw = self.raw_string()?;
        let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        Ok(String::from_utf16_lossy(&units))
    }
}

/// Bytes a Delphi set of `n` members takes: one per 8, with 3 padded to 4.
fn set_bytes(n: usize) -> usize {
    match n.div_ceil(8) {
        3 => 4,
        b => b,
    }
}

/// (number of TSetupHeader options, index of shEncryptionUsed) for a Unicode installer of `v` >= 5.5.0.
fn header_options(v: u32) -> (usize, usize) {
    // DisableStartupPrompt, CreateAppDir, AllowNoIcons, AlwaysRestart, AlwaysUsePersonalGroup, WindowVisible,
    // WindowShowCaption, WindowResizable, WindowStartMaximized, EnableDirDoesntExistWarning, Password,
    // AllowRootDirectory, DisableFinishedPage
    let mut n = 13;
    if v < ver(5, 6, 1, 0) {
        n += 1; // ChangesAssociations
    }
    n += 2; // UsePreviousAppDir, BackColorHorizontal
    // UsePreviousGroup .. AppendDefaultGroupName (21), then EncryptionUsed
    n += 21;
    let encryption = n;
    n += 1;
    if v < ver(5, 6, 1, 0) {
        n += 1; // ChangesEnvironment
    }
    n += 7; // SetupLogging, SignedUninstaller, UsePreviousLanguage, DisableWelcomePage, Close/RestartApplications, AllowNetworkDrive
    if v >= ver(5, 5, 7, 0) {
        n += 1; // ForceCloseApplications
    }
    if v >= ver(6, 0, 0, 0) {
        n += 3; // AppNameHasConsts, UsePreviousPrivileges, WizardResizable
    }
    if v >= ver(6, 3, 0, 0) {
        n += 1; // UninstallLogging
    }
    (n, encryption)
}

/// TSetupVersionData pair (MinVersion, OnlyBelowVersion).
const WINVER: usize = 20;

fn parse_headers(primary: &[u8], secondary: &[u8], v: u32) -> Result<Installer, Error> {
    let bad = |what: &str, e: Error| Error::Invalid(format!("unexpected installer layout ({what}: {e}); try innoextract"));
    let mut c = Cur { b: primary, pos: 0 };
    let app_name = c.wstring().map_err(|e| bad("header", e))?;
    let mut strings = 26 - 1; // AppVerName .. Uninstallable (Inno 5.3.10+), after AppName
    if v >= ver(5, 5, 0, 0) {
        strings += 1; // CloseApplicationsFilter
    }
    if v >= ver(5, 5, 6, 0) {
        strings += 1; // SetupMutex
    }
    if v >= ver(5, 6, 1, 0) {
        strings += 2; // ChangesEnvironment, ChangesAssociations
    }
    if v >= ver(6, 3, 0, 0) {
        strings += 2; // ArchitecturesAllowed, ArchitecturesInstallIn64BitMode
    }
    strings += 4; // LicenseText, InfoBeforeText, InfoAfterText, CompiledCodeText
    c.skip_strings(strings).map_err(|e| bad("header strings", e))?;
    let counts: Vec<u32> = (0..16).map(|_| c.u32()).collect::<Result<_, _>>().map_err(|e| bad("header counts", e))?;
    let [languages, messages, permissions, types, components, tasks, dirs, files, data_entries, ..] = counts[..] else {
        unreachable!()
    };
    let mut fixed = || -> Result<(u8, u32, bool), Error> {
        c.skip(WINVER + 8)?; // MinVersion, OnlyBelowVersion, BackColor, BackColor2
        if v < ver(5, 5, 7, 0) {
            c.skip(4)?; // WizardImageBackColor
        }
        if v >= ver(6, 0, 0, 0) {
            c.skip(9)?; // WizardStyle, WizardResizePercentX/Y
        }
        if v >= ver(5, 5, 7, 0) {
            c.skip(1)?; // WizardImageAlphaFormat
        }
        c.skip(20 + 8)?; // PasswordHash, PasswordSalt
        c.skip(8)?; // ExtraDiskSpaceRequired
        let slices_per_disk = c.u32()?;
        c.skip(3)?; // UninstallLogMode, DirExistsWarning, PrivilegesRequired
        if v >= ver(5, 7, 0, 0) {
            c.skip(1)?; // PrivilegesRequiredOverridesAllowed
        }
        c.skip(2)?; // ShowLanguageDialog, LanguageDetectionMethod
        let compression = c.u8()?;
        if v < ver(6, 3, 0, 0) {
            c.skip(2)?; // ArchitecturesAllowed, ArchitecturesInstallIn64BitMode
        }
        c.skip(2 + 8)?; // DisableDirPage, DisableProgramGroupPage, UninstallDisplaySize
        let (n, enc) = header_options(v);
        let options = c.bytes(set_bytes(n))?;
        Ok((compression, slices_per_disk, options[enc / 8] & (1 << (enc % 8)) != 0))
    };
    let (compression, slices_per_disk, _encrypted) = fixed().map_err(|e| bad("header fields", e))?;
    let compression = match compression {
        0 => Compression::Stored,
        1 => Compression::Zlib,
        2 => Compression::Bzip2,
        3 => Compression::Lzma1,
        4 => Compression::Lzma2,
        n => return Err(Error::Invalid(format!("unknown compression method {n}"))),
    };

    // Entries before the files: (count, strings, fixed bytes after them).
    let skip_entries = |c: &mut Cur, what: &str, count: u32, strings: usize, fixed: usize| -> Result<(), Error> {
        for _ in 0..count {
            c.skip_strings(strings).and_then(|_| c.skip(fixed)).map_err(|e| bad(what, e))?;
        }
        Ok(())
    };
    skip_entries(&mut c, "languages", languages, 10, 21)?;
    skip_entries(&mut c, "messages", messages, 2, 4)?;
    skip_entries(&mut c, "permissions", permissions, 1, 0)?;
    skip_entries(&mut c, "types", types, 4, WINVER + 1 + 1 + 8)?;
    skip_entries(&mut c, "components", components, 5, 8 + 4 + 1 + WINVER + 1 + 8)?;
    skip_entries(&mut c, "tasks", tasks, 6, 4 + 1 + WINVER + 1)?;
    skip_entries(&mut c, "directories", dirs, 7, 4 + WINVER + 2 + 1)?;

    let mut file_entries = Vec::with_capacity(files.min(1 << 16) as usize);
    for _ in 0..files {
        let mut entry = || -> Result<FileEntry, Error> {
            c.raw_string()?; // SourceFilename
            let dest = c.wstring()?;
            c.skip_strings(2)?; // InstallFontName, StrongAssemblyName
            c.skip_strings(2)?; // Components, Tasks
            let languages = c.wstring()?;
            c.skip_strings(3)?; // Check, AfterInstall, BeforeInstall
            c.skip(WINVER)?;
            let location = c.u32()?;
            c.skip(4 + 8 + 2 + set_bytes(32) + 1)?; // Attribs, ExternalSize, PermissionsEntry, Options, FileType
            Ok(FileEntry { dest, languages, location })
        };
        file_entries.push(entry().map_err(|e| bad("files", e))?);
    }

    // File locations.
    let flags = if (ver(5, 5, 7, 0)..ver(6, 3, 0, 0)).contains(&v) { 11 } else { 9 };
    let entry_size = 4 * 3 + 8 * 3 + 20 + 8 + 8 + set_bytes(flags) + usize::from(v >= ver(6, 3, 0, 0));
    if secondary.len() != data_entries as usize * entry_size {
        return Err(bad(
            "file table",
            Error::Invalid(format!("{} bytes for {data_entries} entries of {entry_size}", secondary.len())),
        ));
    }
    let mut d = Cur { b: secondary, pos: 0 };
    let mut data = Vec::with_capacity(data_entries as usize);
    for _ in 0..data_entries {
        let mut entry = || -> Result<DataEntry, Error> {
            let first_slice = d.u32()?;
            let _last_slice = d.u32()?;
            let chunk_offset = d.u32()?;
            let file_offset = d.u64()?;
            let file_size = d.u64()?;
            let chunk_size = d.u64()?;
            let sha1 = d.bytes(20)?.try_into().unwrap();
            d.skip(8 + 8)?; // SourceTimeStamp, FileVersionMS/LS
            let f = d.bytes(set_bytes(flags))?;
            let bit = |i: usize| f[i / 8] & (1 << (i % 8)) != 0;
            if v >= ver(6, 3, 0, 0) {
                d.skip(1)?; // Sign
            }
            Ok(DataEntry {
                first_slice,
                chunk_offset,
                file_offset,
                file_size,
                chunk_size,
                sha1,
                filtered: bit(4),
                encrypted: bit(6),
                compressed: bit(7),
            })
        };
        data.push(entry().map_err(|e| bad("file table", e))?);
    }
    Ok(Installer {
        path: PathBuf::new(),
        version_text: String::new(),
        app_name,
        compression,
        slices_per_disk: slices_per_disk.max(1),
        data_offset: 0,
        files: file_entries,
        data,
    })
}

/// Reads the file data across the .exe's embedded payload or the `-N.bin` parts.
struct SliceReader {
    exe: PathBuf,
    embedded_at: u32,
    slices_per_disk: u32,
    file: Option<(u32, BufReader<File>, u64)>,
}

impl SliceReader {
    fn new(inst: &Installer) -> Result<SliceReader, Error> {
        Ok(SliceReader { exe: inst.path.clone(), embedded_at: inst.data_offset, slices_per_disk: inst.slices_per_disk, file: None })
    }

    /// `setup_x.exe` -> `setup_x-1.bin` (or `-1a.bin` .. with several slices per disk), matched case-insensitively.
    fn slice_path(&self, slice: u32) -> Result<PathBuf, Error> {
        let stem = self.exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let name = if self.slices_per_disk <= 1 {
            format!("{stem}-{}.bin", slice + 1)
        } else {
            let minor = (b'a' + (slice % self.slices_per_disk) as u8) as char;
            format!("{stem}-{}{minor}.bin", slice / self.slices_per_disk + 1)
        };
        let dir = self.exe.parent().unwrap_or(Path::new("."));
        if let Ok(p) = crate::find_in_dir(dir, &name) {
            return Ok(p);
        }
        Err(Error::Invalid(format!(
            "this installer keeps its data in {name}, which isn't next to it in {}; download it from GOG too and put \
             it in the same folder",
            dir.display()
        )))
    }

    fn open(&mut self, slice: u32) -> Result<(), Error> {
        if self.file.as_ref().is_some_and(|(s, _, _)| *s == slice) {
            return Ok(());
        }
        if self.embedded_at != 0 {
            if slice != 0 {
                return Err(Error::Invalid("file data refers past the installer's single part".into()));
            }
            let f = File::open(&self.exe).map_err(|e| io_err(&self.exe, e))?;
            let len = f.metadata().map_err(|e| io_err(&self.exe, e))?.len();
            self.file = Some((0, BufReader::with_capacity(1 << 16, f), len));
            return Ok(());
        }
        let path = self.slice_path(slice)?;
        let mut f = File::open(&path).map_err(|e| io_err(&path, e))?;
        let len = f.metadata().map_err(|e| io_err(&path, e))?.len();
        let mut h = [0u8; 12];
        f.read_exact(&mut h).map_err(|e| io_err(&path, e))?;
        let size = u32::from_le_bytes(h[8..12].try_into().unwrap()) as u64;
        if !SLICE_IDS.iter().any(|id| &h[..8] == *id) || size > len || size < 12 {
            return Err(Error::Invalid(format!("{} isn't a part of this installer", path.display())));
        }
        self.file = Some((slice, BufReader::with_capacity(1 << 16, f), size));
        Ok(())
    }

    fn seek(&mut self, slice: u32, offset: u32) -> Result<(), Error> {
        self.open(slice)?;
        let base = if self.embedded_at != 0 { self.embedded_at as u64 } else { 0 };
        let (_, f, size) = self.file.as_mut().unwrap();
        if base + offset as u64 > *size {
            return Err(Error::Invalid("file data offset past the end of the installer".into()));
        }
        f.seek(SeekFrom::Start(base + offset as u64)).map_err(|e| io_err(&self.exe, e))?;
        Ok(())
    }
}

impl Read for SliceReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let Some((slice, f, size)) = self.file.as_mut() else { return Ok(0) };
            let pos = f.stream_position()?;
            if pos < *size {
                let n = buf.len().min((*size - pos) as usize);
                return f.read(&mut buf[..n]);
            }
            if self.embedded_at != 0 {
                return Ok(0);
            }
            // Continue in the next part, after its 12-byte header.
            let next = *slice + 1;
            self.open(next).map_err(|e| io::Error::other(e.to_string()))?;
        }
    }
}

/// One file being written from a chunk.
struct PendingFile {
    offset: u64,
    size: u64,
    outputs: Vec<PathBuf>,
    filtered: bool,
    sha1: [u8; 20],
    /// The whole file, for filtered files; else the first output, written as it arrives.
    buf: Vec<u8>,
    out: Option<io::BufWriter<File>>,
    hash: sha1_smol::Sha1,
    got: u64,
}

impl PendingFile {
    fn new(e: &DataEntry, outputs: Vec<PathBuf>) -> PendingFile {
        PendingFile {
            offset: e.file_offset,
            size: e.file_size,
            outputs,
            filtered: e.filtered,
            sha1: e.sha1,
            buf: Vec::new(),
            out: None,
            hash: sha1_smol::Sha1::new(),
            got: 0,
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.filtered {
            self.buf.extend_from_slice(data);
        } else {
            if self.out.is_none() {
                self.out = Some(io::BufWriter::new(create(&self.outputs[0])?));
            }
            self.hash.update(data);
            self.out.as_mut().unwrap().write_all(data).map_err(|e| io_err(&self.outputs[0], e))?;
        }
        self.got += data.len() as u64;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        let first = &self.outputs[0];
        if self.filtered {
            unfilter_calls(&mut self.buf);
            self.hash.update(&self.buf);
            create(first)?.write_all(&self.buf).map_err(|e| io_err(first, e))?;
            self.buf = Vec::new();
        } else {
            match self.out.take() {
                Some(mut w) => w.flush().map_err(|e| io_err(first, e))?,
                None => drop(create(first)?), // empty file
            }
        }
        if self.hash.digest().bytes() != self.sha1 {
            let _ = fs::remove_file(first);
            return Err(Error::Invalid(format!(
                "{}: checksum mismatch after extraction (damaged download, or an installer variant op2-rs reads \
                 wrongly; innoextract can extract it)",
                first.display()
            )));
        }
        for other in &self.outputs[1..] {
            if let Some(parent) = other.parent() {
                fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
            }
            fs::copy(first, other).map_err(|e| io_err(other, e))?;
        }
        Ok(())
    }
}

fn create(path: &Path) -> Result<File, Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    File::create(path).map_err(|e| io_err(path, e))
}

/// Routes a chunk's decompressed bytes into its files. Stops the decoder (with an error) once every file is done.
struct ChunkSink<'a> {
    pos: u64,
    files: Vec<PendingFile>,
    next: usize,
    needed: u64,
    written: u64,
    total: u64,
    progress: &'a mut dyn FnMut(u64, u64),
    error: Option<Error>,
}

impl ChunkSink<'_> {
    fn feed(&mut self, mut data: &[u8]) -> Result<(), Error> {
        while !data.is_empty() && self.next < self.files.len() {
            let f = &mut self.files[self.next];
            if f.offset + f.size < self.pos || (f.offset < self.pos && f.got == 0) {
                if f.size != 0 {
                    return Err(Error::Invalid("overlapping files in an installer chunk".into()));
                }
                f.finish()?;
                self.next += 1;
                continue;
            }
            if self.pos < f.offset {
                let skip = data.len().min((f.offset - self.pos) as usize);
                self.pos += skip as u64;
                data = &data[skip..];
                continue;
            }
            let take = data.len().min((f.offset + f.size - self.pos) as usize);
            f.write(&data[..take])?;
            self.pos += take as u64;
            self.written += take as u64;
            data = &data[take..];
            if f.got == f.size {
                f.finish()?;
                self.next += 1;
            }
        }
        // Zero-length files at the current position.
        while self.next < self.files.len() && self.files[self.next].size == 0 && self.files[self.next].offset <= self.pos {
            self.files[self.next].finish()?;
            self.next += 1;
        }
        (self.progress)(self.written, self.total);
        Ok(())
    }
}

impl Write for ChunkSink<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.next >= self.files.len() || self.pos >= self.needed {
            return Err(io::Error::other("chunk complete"));
        }
        if let Err(e) = self.feed(data) {
            let msg = e.to_string();
            self.error = Some(e);
            return Err(io::Error::other(msg));
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Decompresses one chunk into `sink`. An error after the sink has everything it needs is not a failure (the sink
/// stops the decoder that way); the caller checks the sink.
fn decompress(method: Compression, mut input: impl Read, sink: &mut ChunkSink, needed: u64) -> Result<(), Error> {
    let err = |e: &dyn std::fmt::Display| Error::Invalid(e.to_string());
    match method {
        Compression::Stored => {
            io::copy(&mut input, sink).map_err(|e| err(&e))?;
        }
        Compression::Zlib => {
            io::copy(&mut flate2::read::ZlibDecoder::new(input), sink).map_err(|e| err(&e))?;
        }
        Compression::Lzma1 => {
            let mut props = [0u8; 5];
            input.read_exact(&mut props).map_err(|e| err(&e))?;
            let mut r = lzma1_reader(input, &props, needed)?;
            io::copy(&mut r, sink).map_err(|e| err(&e))?;
        }
        Compression::Lzma2 => {
            let mut prop = [0u8; 1];
            input.read_exact(&mut prop).map_err(|e| err(&e))?;
            let dict = match prop[0] {
                0..40 => (2 | (prop[0] as u32 & 1)) << (prop[0] / 2 + 11),
                40 => u32::MAX,
                _ => return Err(Error::Invalid("bad LZMA2 properties".into())),
            };
            // The dictionary never needs to be bigger than the data, so a huge declared one costs nothing.
            let dict = dict.min(needed.clamp(4096, u32::MAX as u64) as u32);
            let mut r = lzma_rust2::Lzma2Reader::new(io::BufReader::new(input), dict, None);
            io::copy(&mut r, sink).map_err(|e| err(&e))?;
        }
        Compression::Bzip2 => {
            return Err(Error::Unsupported("bzip2-compressed installers aren't supported; use innoextract".into()));
        }
    }
    Ok(())
}

/// Undoes Inno Setup's x86 branch filter (5.3.9 and later): the compiler turned the 24-bit target of each E8/E9
/// CALL/JMP whose top byte is 00 or FF into an absolute address, flipping the top byte for backward targets, and
/// skipped instructions straddling a 64 KiB block.
fn unfilter_calls(b: &mut [u8]) {
    const BLOCK: usize = 0x10000;
    let mut i = 0;
    while i < b.len() {
        let op = b[i];
        i += 1;
        if op != 0xE8 && op != 0xE9 {
            continue;
        }
        if BLOCK - ((i - 1) % BLOCK) < 5 {
            continue;
        }
        if i + 4 > b.len() {
            break;
        }
        if b[i + 3] == 0x00 || b[i + 3] == 0xFF {
            let addr = ((i + 4) as u32) & 0xFF_FFFF;
            let rel = (b[i] as u32 | (b[i + 1] as u32) << 8 | (b[i + 2] as u32) << 16).wrapping_sub(addr);
            b[i] = rel as u8;
            b[i + 1] = (rel >> 8) as u8;
            b[i + 2] = (rel >> 16) as u8;
            if rel & 0x80_0000 != 0 {
                b[i + 3] = !b[i + 3];
            }
        }
        i += 4;
    }
}

fn io_err(path: &Path, source: io::Error) -> Error {
    Error::Io { path: path.display().to_string(), source }
}

impl Error {
    fn context(self, what: &str) -> Error {
        Error::Invalid(format!("{what}: {self}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("Inno Setup Setup Data (5.6.2) (u)"), Some(ver(5, 6, 2, 0)));
        assert_eq!(parse_version("Inno Setup Setup Data (6.4.0.1)"), Some(ver(6, 4, 0, 1)));
        assert_eq!(parse_version("Inno Setup Setup Data (5.5.7) (U)"), Some(ver(5, 5, 7, 0)));
        assert_eq!(parse_version("Inno Setup Setup Data (x)"), None);
        assert_eq!(parse_version("My Inno Setup Extensions Setup Data (3.0.4)"), None);
    }

    #[test]
    fn header_option_counts_match_struct_pas() {
        // Inno Setup 5.6.1's Struct.pas (Unicode) lists 47 options with shEncryptionUsed the 38th.
        assert_eq!(header_options(ver(5, 5, 7, 0)), (47, 37));
        // GOG's 5.6.2 drops ChangesAssociations/ChangesEnvironment (now strings); 6.0 adds three.
        assert_eq!(header_options(ver(5, 6, 2, 0)), (45, 36));
        assert_eq!(header_options(ver(6, 0, 0, 0)).0, 48);
        assert_eq!(set_bytes(47), 6);
        assert_eq!(set_bytes(17), 4);
        assert_eq!(set_bytes(32), 4);
        assert_eq!(set_bytes(11), 2);
    }

    #[test]
    fn destinations_stay_inside_the_game_folder() {
        assert_eq!(app_relative("{app}\\maps.vol"), Some(PathBuf::from("maps.vol")));
        assert_eq!(app_relative("{APP}\\OPU\\base\\x.ini"), Some(["OPU", "base", "x.ini"].iter().collect()));
        assert_eq!(app_relative("{tmp}\\setup.dll"), None);
        assert_eq!(app_relative("{app}\\..\\evil.dll"), None);
        assert_eq!(app_relative("{app}\\C:\\evil.dll"), None);
        assert_eq!(app_relative("{app}\\{sys}\\x.dll"), None);
        assert_eq!(app_relative("{app}\\"), None);
        assert_eq!(app_relative("{app}"), None);
    }

    #[test]
    fn offset_table_crc_is_checked() {
        let mut t = Vec::new();
        t.extend_from_slice(OFFSET_TABLE_IDS[0]);
        for v in [1u32, 1000, 0x100, 50, 0, 0x2000, 0x3000] {
            t.extend_from_slice(&v.to_le_bytes());
        }
        let crc = crc32fast::hash(&t);
        t.extend_from_slice(&crc.to_le_bytes());
        assert_eq!(parse_offset_table(&t), Some((0x2000, 0x3000)));
        t[20] ^= 1;
        assert_eq!(parse_offset_table(&t), None);
    }

    /// Re-applies the compiler's filter (the inverse of `unfilter_calls`), to check the round trip.
    fn filter_calls(b: &mut [u8]) {
        let mut i = 0;
        while i < b.len() {
            let op = b[i];
            i += 1;
            if (op != 0xE8 && op != 0xE9) || 0x10000 - ((i - 1) % 0x10000) < 5 {
                continue;
            }
            if i + 4 > b.len() {
                break;
            }
            if b[i + 3] == 0x00 || b[i + 3] == 0xFF {
                let addr = ((i + 4) as u32) & 0xFF_FFFF;
                let rel = b[i] as u32 | (b[i + 1] as u32) << 8 | (b[i + 2] as u32) << 16;
                let abs = rel.wrapping_add(addr);
                if rel & 0x80_0000 != 0 {
                    b[i + 3] = !b[i + 3];
                }
                b[i] = abs as u8;
                b[i + 1] = (abs >> 8) as u8;
                b[i + 2] = (abs >> 16) as u8;
            }
            i += 4;
        }
    }

    #[test]
    fn call_filter_round_trips() {
        // Pseudo-random bytes dense in E8/E9, across a 64 KiB block boundary.
        let mut x = 0x1234_5678u32;
        let orig: Vec<u8> = (0..0x10000 + 300)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                match i % 7 {
                    0 => 0xE8,
                    3 if x & 1 == 0 => 0xFF,
                    3 => 0x00,
                    _ => x as u8,
                }
            })
            .collect();
        let mut b = orig.clone();
        filter_calls(&mut b);
        assert_ne!(b, orig);
        unfilter_calls(&mut b);
        assert_eq!(b, orig);
    }
}
