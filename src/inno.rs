//! Extracting the game folder from GOG's offline installer (`setup_outpost_2_*.exe`, plus any `-N.bin` parts next to
//! it), an Inno Setup installer, so a player who only kept the download can play without running it under Wine.
//! Only files the player points at are read; nothing is downloaded (docs/legal.md rule 1).
//!
//! Supports the Unicode installers of Inno Setup 5.5.0 to 6.3 (GOG's Outpost 2 installer says "5.6.2", a GOG build
//! between 5.6.1 and 6.0). The layout follows Inno Setup's own `Struct.pas` and innoextract's notes on how it
//! changed between versions; everything read is untrusted and bounds-checked. Encrypted and bzip2-compressed
//! installers aren't supported: innoextract handles those (docs/game-data.md).
//!
//! GOG's current installers ("GOG Galaxy" builds, e.g. `setup_outpost_2_1.2_(37379).exe`) don't install the game
//! with plain `{app}` entries. Each game file is cut into parts of up to 10 MiB, each part zlib-compressed on its own
//! and stored as an Inno file entry with a `{tmp}\xx\yy\<md5>` destination; the installer's Pascal code reassembles
//! them, driven by strings in the entries' `BeforeInstall`/`AfterInstall`/`Check` parameters:
//!
//! - `before_install('<md5 of the file>', '<path below {app}>', <number of parts>)` on a file's first part;
//! - `after_install('<md5 of the part>', <stored size>, <inflated size>)` on every part, the first included;
//! - `check_if_install('<languages>', '<32#64#>', '<os>')`, `#`-separated, `!` negating (e.g. `en-US#`);
//! - `before_install_dependency`/`after_install_dependency`/`check_if_install_dependency` for redistributables
//!   (`__redist\...`, DirectX here), which aren't part of the game and are skipped.
//!
//! The meaning of these calls was learned from innoextract's `src/cli/goggalaxy.cpp` (1.8+), and checked against
//! the Outpost 2 installer: the parts' MD5s are of the inflated data, a one-part file's part MD5 equals its file
//! MD5, and the stored size equals the Inno data entry's size. Galaxy files are checked three ways: the Inno SHA-1
//! of the stored data, the MD5 of each inflated part, and the MD5 of the whole file.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::Error;
use crate::md5::Md5;

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

#[derive(Clone, Debug, Default)]
struct FileEntry {
    /// Destination as written in the script, e.g. `{app}\maps.vol`.
    dest: String,
    languages: String,
    /// The `Check`, `BeforeInstall` and `AfterInstall` parameters (Pascal expressions; GOG Galaxy's calls, above).
    check: String,
    before: String,
    after: String,
    /// Index into the data entries, or `u32::MAX` for files not stored in the installer.
    location: u32,
}

/// One part of an installed file: a data entry's bytes, as stored or inflated.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Part {
    data: usize,
    /// GOG Galaxy parts: the stored data is a zlib stream inflating to this many bytes, with this MD5.
    inflate: Option<(u64, Option<[u8; 16]>)>,
}

/// A file installed below `{app}`, made of its parts in order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    rel: PathBuf,
    parts: Vec<Part>,
    size: u64,
    /// MD5 of the whole file (GOG Galaxy files).
    md5: Option<[u8; 16]>,
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
    data: Vec<DataEntry>,
    /// What gets installed below `{app}`, in the installer's order.
    targets: Vec<Target>,
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
        self.targets.iter().map(|t| (t.rel.clone(), t.size)).collect()
    }

    /// Total bytes [`extract`](Self::extract) writes.
    pub fn total_size(&self) -> u64 {
        self.targets.iter().map(|t| t.size).sum()
    }

    /// Extracts the game folder into `out` (created if needed), checking every file against the installer's
    /// checksums. `progress` gets (bytes written, total) as it goes.
    pub fn extract(&self, out: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<Extracted, Error> {
        let total = self.total_size();
        // What each data entry becomes (identical data is stored once and may be installed under several names).
        let mut uses: HashMap<usize, Vec<Use>> = HashMap::new();
        for (t, target) in self.targets.iter().enumerate() {
            let path = out.join(&target.rel);
            let multi = target.parts.len() > 1;
            let mut offset = 0;
            for (p, part) in target.parts.iter().enumerate() {
                let size = part.inflate.map_or(self.data[part.data].file_size, |(n, _)| n);
                uses.entry(part.data).or_default().push(Use {
                    target: t,
                    part: p,
                    path: path.clone(),
                    offset,
                    size,
                    multi,
                    inflate: part.inflate.is_some(),
                    md5: part.inflate.and_then(|(_, m)| m),
                });
                offset += size;
            }
            if multi {
                // Parts may come in any order, so the file is sized up front and each part written in place.
                create(&path)?.set_len(target.size).map_err(|e| io_err(&path, e))?;
            }
        }
        // Chunks in file order, each with its data entries sorted by offset.
        let mut chunks: Vec<((u32, u32), Vec<usize>)> = Vec::new();
        for &d in uses.keys() {
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

        // Chunks are independent, so they're extracted on several threads. A multi-part file's whole-file MD5 is
        // computed as its parts are written in order, so chunks sharing a multi-part file form one job, done in file
        // order by one thread.
        let mut group: Vec<usize> = (0..chunks.len()).collect();
        let mut first_chunk: HashMap<usize, usize> = HashMap::new();
        for (c, (_, entries)) in chunks.iter().enumerate() {
            for u in entries.iter().flat_map(|d| &uses[d]).filter(|u| u.multi) {
                let g = *first_chunk.entry(u.target).or_insert(group[c]);
                let old = group[c];
                for x in group.iter_mut().filter(|x| **x == old) {
                    *x = g;
                }
            }
        }
        let mut jobs: Vec<(u64, Vec<ChunkJob>)> = Vec::new();
        let mut job_of: HashMap<usize, usize> = HashMap::new();
        for (c, (_, entries)) in chunks.iter().enumerate() {
            let j = *job_of.entry(group[c]).or_insert_with(|| {
                jobs.push((0, Vec::new()));
                jobs.len() - 1
            });
            let files = entries.iter().map(|&d| PendingFile::new(&self.data[d], uses.remove(&d).unwrap_or_default())).collect();
            jobs[j].0 += self.data[entries[0]].chunk_size;
            jobs[j].1.push(ChunkJob { first: entries[0], files });
        }
        // Biggest last: workers pop from the end, so the long jobs start first.
        jobs.sort_by_key(|j| j.0);
        let queue = Mutex::new(jobs);
        let states = Mutex::new(
            self.targets
                .iter()
                .map(|t| TargetState { next_part: 0, md5: (t.parts.len() > 1 && t.md5.is_some()).then(Md5::new), in_order: true })
                .collect::<Vec<_>>(),
        );
        let written = AtomicU64::new(0);
        let failed = AtomicBool::new(false);
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4);
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let results: Vec<Result<(), Error>> = std::thread::scope(|s| {
            let workers: Vec<_> = (0..threads)
                .map(|_| {
                    let tx = tx.clone();
                    let (queue, states, written, failed) = (&queue, &states, &written, &failed);
                    s.spawn(move || {
                        let r = self.extract_jobs(queue, states, written, failed);
                        if r.is_err() {
                            failed.store(true, Ordering::Relaxed);
                        }
                        drop(tx);
                        r
                    })
                })
                .collect();
            drop(tx);
            // Progress from this thread (the callback needn't be Send) until every worker is done.
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(std::time::Duration::from_millis(50)) {
                progress(written.load(Ordering::Relaxed), total);
            }
            workers.into_iter().map(|w| w.join().unwrap_or_else(|p| std::panic::resume_unwind(p))).collect()
        });
        results.into_iter().collect::<Result<Vec<()>, Error>>()?;
        let mut done = Extracted { bytes: written.load(Ordering::Relaxed), ..Extracted::default() };
        progress(done.bytes, total);
        let states = states.into_inner().unwrap_or_else(|e| e.into_inner());
        // Multi-part files as a whole (a one-part file's MD5 is its part's, checked already).
        for (t, s) in self.targets.iter().zip(states) {
            let Some(want) = t.md5.filter(|_| t.parts.len() > 1) else { continue };
            let path = out.join(&t.rel);
            let got = match s.md5 {
                Some(m) if s.in_order && s.next_part == t.parts.len() => m.digest(),
                _ => md5_file(&path)?,
            };
            if got != want {
                let _ = fs::remove_file(&path);
                return Err(mismatch(&path));
            }
        }
        done.files = self.targets.len();
        Ok(done)
    }

    fn io(&self, e: io::Error) -> Error {
        io_err(&self.path, e)
    }

    /// A worker of [`Installer::extract`]: takes jobs from `queue` until it's empty or another worker failed.
    fn extract_jobs(&self, queue: &Mutex<Vec<(u64, Vec<ChunkJob>)>>, states: &Mutex<Vec<TargetState>>, written: &AtomicU64, failed: &AtomicBool) -> Result<(), Error> {
        let mut slices = SliceReader::new(self)?;
        loop {
            let Some((_, job)) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop() else { return Ok(()) };
            for chunk in job {
                if failed.load(Ordering::Relaxed) {
                    return Ok(());
                }
                self.extract_chunk(&mut slices, chunk, states, written)?;
            }
        }
    }

    /// Decompresses one chunk into its files.
    fn extract_chunk(&self, slices: &mut SliceReader, chunk: ChunkJob, states: &Mutex<Vec<TargetState>>, written: &AtomicU64) -> Result<(), Error> {
        let first = &self.data[chunk.first];
        slices.seek(first.first_slice, first.chunk_offset)?;
        let mut id = [0u8; 4];
        slices.read_exact(&mut id).map_err(|e| self.io(e))?;
        if &id != CHUNK_ID {
            return Err(Error::Invalid(format!("{}: bad chunk header at {:#x}", self.path.display(), first.chunk_offset)));
        }
        let needed = chunk.files.iter().map(|f| f.offset + f.size).max().unwrap_or(0);
        let mut sink = ChunkSink { pos: 0, files: chunk.files, next: 0, needed, written, states, error: None };
        let compressed = slices.take(first.chunk_size);
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
            let name = sink.files[sink.next].uses.first().map(|u| u.path.display().to_string()).unwrap_or_default();
            let why = result.err().map(|e| e.to_string()).unwrap_or_else(|| "data ended early".into());
            return Err(Error::Invalid(format!("{}: decompressing {name}: {why}", self.path.display())));
        }
        Ok(())
    }
}

/// One chunk to extract: its first data entry (where the chunk is) and its files, in offset order.
struct ChunkJob {
    first: usize,
    files: Vec<PendingFile>,
}

/// `{app}\sub\file` as a safe relative path (`sub/file`); `None` for other destinations or anything that could
/// escape the output folder.
fn app_relative(dest: &str) -> Option<PathBuf> {
    let rest = dest.get(..6).filter(|p| p.eq_ignore_ascii_case("{app}\\")).map(|_| &dest[6..])?;
    safe_relative(rest)
}

/// `sub\file` as a relative path that stays inside the output folder.
fn safe_relative(rest: &str) -> Option<PathBuf> {
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

/// The files installed below `{app}`: plain `{app}` entries and reassembled GOG Galaxy files (module docs). When
/// several entries write the same file (per-language variants), the English or language-neutral one wins.
fn plan(files: &[FileEntry], data: &[DataEntry]) -> Result<Vec<Target>, Error> {
    let bad = |m: String| Error::Invalid(format!("unexpected GOG installer layout ({m}); try innoextract"));
    let mut out: Vec<(Target, u8)> = Vec::new();
    let mut by_dest: HashMap<String, usize> = HashMap::new();
    let mut add = |t: Target, rank: u8| {
        let key = t.rel.to_string_lossy().to_lowercase();
        match by_dest.get(&key) {
            Some(&i) if rank > out[i].1 => out[i] = (t, rank),
            Some(_) => {}
            None => {
                by_dest.insert(key, out.len());
                out.push((t, rank));
            }
        }
    };
    // The Galaxy file being assembled: the file so far, parts still to come, and its rank if it is installed.
    let mut galaxy: Option<(Target, u32, Option<u8>)> = None;
    for (i, f) in files.iter().enumerate() {
        let location = f.location as usize;
        let start = galaxy_call(&f.before, "before_install")
            .map(|a| (a, false))
            .or_else(|| galaxy_call(&f.before, "before_install_dependency").map(|a| (a, true)));
        if let Some((args, dependency)) = start {
            if let Some((t, left, _)) = &galaxy {
                return Err(bad(format!("{} lacks {left} of its parts", t.rel.display())));
            }
            let [md5, name, count, ..] = &args[..] else { return Err(bad(format!("file {i}: {}", f.before))) };
            let count = count.parse::<u32>().map_err(|_| bad(format!("file {i}: {}", f.before)))?.max(1);
            let rel = safe_relative(name);
            let rank = match (&rel, dependency) {
                (_, true) => None,
                (None, false) => return Err(bad(format!("file {i}: unsafe path {name:?}"))),
                (Some(_), false) => {
                    let check = galaxy_call(&f.check, "check_if_install").unwrap_or_default();
                    let arg = |n: usize| check.get(n).map_or("", |s| s.as_str());
                    galaxy_has(arg(0), |l| l.eq_ignore_ascii_case("en") || l.to_ascii_lowercase().starts_with("en-"))
                        .then(|| 1 + u8::from(galaxy_has(arg(1), |a| a == "64")))
                }
            };
            let target = Target { rel: rel.unwrap_or_default(), parts: Vec::new(), size: 0, md5: crate::md5::parse_hex(md5) };
            galaxy = Some((target, count, rank));
        }
        let part = galaxy_call(&f.after, "after_install").or_else(|| galaxy_call(&f.after, "after_install_dependency"));
        if let Some(args) = part {
            let Some((t, left, rank)) = galaxy.as_mut() else { return Err(bad(format!("file {i} is a part of nothing"))) };
            *left -= 1;
            // What isn't installed (redistributables, other languages) only has to be well-formed enough to step over.
            if rank.is_some() {
                let [md5, stored, size, ..] = &args[..] else { return Err(bad(format!("file {i}: {}", f.after))) };
                let (Ok(stored), Ok(size)) = (stored.parse::<u64>(), size.parse::<u64>()) else {
                    return Err(bad(format!("file {i}: {}", f.after)));
                };
                if data.get(location).map(|d| d.file_size) != Some(stored) {
                    return Err(bad(format!("a part of {} isn't the {stored} bytes it says", t.rel.display())));
                }
                t.parts.push(Part { data: location, inflate: Some((size, crate::md5::parse_hex(md5))) });
                t.size = t.size.checked_add(size).ok_or_else(|| bad(format!("{} is too big", t.rel.display())))?;
            }
            if *left == 0 {
                let (mut t, _, rank) = galaxy.take().unwrap();
                if let [Part { inflate: Some((_, part_md5)), .. }] = &mut t.parts[..] {
                    match (*part_md5, t.md5) {
                        (Some(p), Some(w)) if p != w => {
                            return Err(bad(format!("{}: its only part's MD5 isn't the file's", t.rel.display())));
                        }
                        (None, w) => *part_md5 = w,
                        _ => {}
                    }
                }
                if let Some(rank) = rank {
                    add(t, rank);
                }
            }
            continue;
        }
        let Some(rel) = app_relative(&f.dest) else { continue };
        let Some(d) = data.get(location) else { continue };
        let langs = f.languages.to_ascii_lowercase();
        let preferred = langs.is_empty() || langs.split([' ', ',']).any(|l| l == "english" || l == "en");
        add(Target { rel, parts: vec![Part { data: location, inflate: None }], size: d.file_size, md5: None }, u8::from(preferred));
    }
    if let Some((t, left, _)) = galaxy {
        return Err(bad(format!("{} lacks {left} of its parts", t.rel.display())));
    }
    Ok(out.into_iter().map(|(t, _)| t).collect())
}

/// The arguments of a Pascal call `name('text', 12, False)` as GOG Galaxy installers write them (`''` escapes a
/// quote); `None` for anything else.
fn galaxy_call(expr: &str, name: &str) -> Option<Vec<String>> {
    let mut chars = expr.trim().strip_prefix(name)?.trim_start().strip_prefix('(')?.chars().peekable();
    let mut args = Vec::new();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut arg = String::new();
        if chars.next_if_eq(&'\'').is_some() {
            loop {
                match chars.next()? {
                    '\'' if chars.next_if_eq(&'\'').is_some() => arg.push('\''),
                    '\'' => break,
                    c => arg.push(c),
                }
            }
        } else {
            while let Some(c) = chars.next_if(|c| !c.is_whitespace() && !"(),'".contains(*c)) {
                arg.push(c);
            }
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        args.push(arg);
        match chars.next()? {
            ',' => {}
            ')' => break,
            _ => return None,
        }
    }
    chars.all(char::is_whitespace).then_some(args)
}

/// Whether a Galaxy `#`-separated list (`en-US#de-DE#`, `32#64#`; `!x` excludes x) admits something `is` matches.
/// An empty list admits everything.
fn galaxy_has(list: &str, is: impl Fn(&str) -> bool) -> bool {
    let items: Vec<&str> = list.split('#').map(str::trim).filter(|s| !s.is_empty()).collect();
    let (excluded, included): (Vec<&str>, Vec<&str>) = items.iter().partition(|s| s.starts_with('!'));
    !excluded.iter().any(|s| is(&s[1..])) && (included.is_empty() || included.iter().any(|s| is(s)))
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
            let check = c.wstring()?;
            let after = c.wstring()?;
            let before = c.wstring()?;
            c.skip(WINVER)?;
            let location = c.u32()?;
            c.skip(4 + 8 + 2 + set_bytes(32) + 1)?; // Attribs, ExternalSize, PermissionsEntry, Options, FileType
            Ok(FileEntry { dest, languages, check, before, after, location })
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
    let targets = plan(&file_entries, &data)?;
    Ok(Installer {
        path: PathBuf::new(),
        version_text: String::new(),
        app_name,
        compression,
        slices_per_disk: slices_per_disk.max(1),
        data_offset: 0,
        data,
        targets,
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

/// One way a data entry is installed: as a whole file, or as a part of one.
struct Use {
    target: usize,
    part: usize,
    path: PathBuf,
    /// Where in the file the part goes, and how many bytes it makes there.
    offset: u64,
    size: u64,
    /// The file has several parts (and was created at its full size beforehand).
    multi: bool,
    /// The stored data is a zlib stream (a GOG Galaxy part), inflating to bytes with this MD5.
    inflate: bool,
    md5: Option<[u8; 16]>,
}

/// Progress of a multi-part file's whole-file MD5, which can be computed on the fly while its parts come in order.
struct TargetState {
    next_part: usize,
    md5: Option<Md5>,
    in_order: bool,
}

/// An output file being written, counting and hashing what goes in.
struct OutFile {
    w: io::BufWriter<File>,
    path: PathBuf,
    written: u64,
    part_md5: Option<Md5>,
    whole_md5: Option<Md5>,
}

impl Write for OutFile {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.w.write_all(data)?;
        self.written += data.len() as u64;
        for m in [&mut self.part_md5, &mut self.whole_md5].into_iter().flatten() {
            m.update(data);
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

enum Writer {
    Stored(OutFile),
    Inflate(flate2::write::ZlibDecoder<OutFile>),
}

impl Writer {
    fn out(&self) -> &OutFile {
        match self {
            Writer::Stored(o) => o,
            Writer::Inflate(z) => z.get_ref(),
        }
    }
}

/// One data entry being read from a chunk and written to its uses.
struct PendingFile {
    offset: u64,
    size: u64,
    filtered: bool,
    sha1: [u8; 20],
    /// The whole entry, for filtered entries (unfiltered once complete); unused otherwise.
    buf: Vec<u8>,
    hash: sha1_smol::Sha1,
    got: u64,
    uses: Vec<Use>,
    /// One per use, opened when the first bytes are ready.
    writers: Vec<Writer>,
}

impl PendingFile {
    fn new(e: &DataEntry, uses: Vec<Use>) -> PendingFile {
        PendingFile {
            offset: e.file_offset,
            size: e.file_size,
            filtered: e.filtered,
            sha1: e.sha1,
            buf: Vec::new(),
            hash: sha1_smol::Sha1::new(),
            got: 0,
            uses,
            writers: Vec::new(),
        }
    }

    fn open(&mut self, states: &Mutex<Vec<TargetState>>) -> Result<(), Error> {
        for u in &self.uses {
            let file = if u.multi {
                let mut f = fs::OpenOptions::new().write(true).open(&u.path).map_err(|e| io_err(&u.path, e))?;
                f.seek(SeekFrom::Start(u.offset)).map_err(|e| io_err(&u.path, e))?;
                f
            } else {
                create(&u.path)?
            };
            let whole_md5 = {
                let s = &mut states.lock().unwrap_or_else(|e| e.into_inner())[u.target];
                s.in_order &= s.next_part == u.part;
                if s.in_order { s.md5.take() } else { None }
            };
            let out = OutFile {
                w: io::BufWriter::with_capacity(1 << 16, file),
                path: u.path.clone(),
                written: 0,
                part_md5: u.md5.map(|_| Md5::new()),
                whole_md5,
            };
            self.writers.push(if u.inflate {
                Writer::Inflate(flate2::write::ZlibDecoder::new(out))
            } else {
                Writer::Stored(out)
            });
        }
        Ok(())
    }

    /// Passes stored bytes on to every use; returns the bytes this added to the output files.
    fn emit(&mut self, data: &[u8]) -> Result<u64, Error> {
        let before: u64 = self.writers.iter().map(|w| w.out().written).sum();
        for w in &mut self.writers {
            let r = match w {
                Writer::Stored(o) => o.write_all(data),
                Writer::Inflate(z) => z.write_all(data),
            };
            r.map_err(|e| Error::Invalid(format!("{}: {e}{}", w.out().path.display(), MISMATCH_HINT)))?;
        }
        Ok(self.writers.iter().map(|w| w.out().written).sum::<u64>() - before)
    }

    fn write(&mut self, data: &[u8], states: &Mutex<Vec<TargetState>>) -> Result<u64, Error> {
        self.got += data.len() as u64;
        if self.filtered {
            self.buf.extend_from_slice(data);
            return Ok(0);
        }
        if self.writers.is_empty() {
            self.open(states)?;
        }
        self.hash.update(data);
        self.emit(data)
    }

    /// Completes and checks every use; returns the bytes this added to the output files.
    fn finish(&mut self, states: &Mutex<Vec<TargetState>>) -> Result<u64, Error> {
        if self.writers.is_empty() {
            self.open(states)?;
        }
        let mut produced = 0;
        if self.filtered {
            let mut buf = std::mem::take(&mut self.buf);
            unfilter_calls(&mut buf);
            self.hash.update(&buf);
            produced = self.emit(&buf)?;
        }
        let sha1_ok = self.hash.digest().bytes() == self.sha1;
        for (u, w) in self.uses.iter().zip(std::mem::take(&mut self.writers)) {
            let mut out = match w {
                Writer::Stored(o) => o,
                Writer::Inflate(z) => {
                    let before = z.get_ref().written;
                    let o = z.finish().map_err(|e| Error::Invalid(format!("{}: {e}{MISMATCH_HINT}", u.path.display())))?;
                    produced += o.written - before;
                    o
                }
            };
            out.flush().map_err(|e| io_err(&u.path, e))?;
            let part_ok = match (out.part_md5.take(), u.md5) {
                (Some(m), Some(want)) => m.digest() == want,
                _ => true,
            };
            if !sha1_ok || !part_ok || out.written != u.size {
                drop(out);
                let _ = fs::remove_file(&u.path);
                return Err(mismatch(&u.path));
            }
            let s = &mut states.lock().unwrap_or_else(|e| e.into_inner())[u.target];
            if s.in_order && s.next_part == u.part {
                s.md5 = out.whole_md5.take();
                s.next_part += 1;
            }
        }
        Ok(produced)
    }
}

const MISMATCH_HINT: &str = " (damaged download, or an installer variant op2-rs reads wrongly; innoextract can extract it)";

fn mismatch(path: &Path) -> Error {
    Error::Invalid(format!("{}: checksum mismatch after extraction{MISMATCH_HINT}", path.display()))
}

/// MD5 of a file already written.
fn md5_file(path: &Path) -> Result<[u8; 16], Error> {
    let mut f = File::open(path).map_err(|e| io_err(path, e))?;
    let mut m = Md5::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| io_err(path, e))?;
        if n == 0 {
            return Ok(m.digest());
        }
        m.update(&buf[..n]);
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
    /// Bytes written to output files so far, by every worker.
    written: &'a AtomicU64,
    states: &'a Mutex<Vec<TargetState>>,
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
                self.written.fetch_add(f.finish(self.states)?, Ordering::Relaxed);
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
            self.written.fetch_add(f.write(&data[..take], self.states)?, Ordering::Relaxed);
            self.pos += take as u64;
            data = &data[take..];
            if f.got == f.size {
                self.written.fetch_add(f.finish(self.states)?, Ordering::Relaxed);
                self.next += 1;
            }
        }
        // Zero-length files at the current position.
        while self.next < self.files.len() && self.files[self.next].size == 0 && self.files[self.next].offset <= self.pos {
            self.written.fetch_add(self.files[self.next].finish(self.states)?, Ordering::Relaxed);
            self.next += 1;
        }
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

    #[test]
    fn galaxy_calls_parse() {
        let args = |e: &str, n: &str| galaxy_call(e, n);
        assert_eq!(
            args("before_install('8d93ca771aba3dbe2cc20ccb6ada32d9', 'MPUL.avi', 2)", "before_install"),
            Some(vec!["8d93ca771aba3dbe2cc20ccb6ada32d9".into(), "MPUL.avi".into(), "2".into()])
        );
        assert_eq!(
            args("check_if_install_dependency('d14f', False)", "check_if_install_dependency"),
            Some(vec!["d14f".into(), "False".into()])
        );
        assert_eq!(args("f('it''s', 'a\\b')", "f"), Some(vec!["it's".into(), "a\\b".into()]));
        // A longer name with the same start isn't the call asked for.
        assert_eq!(args("before_install_dependency('x', 'y', 1)", "before_install"), None);
        assert_eq!(args("not isWinXP", "check_if_install"), None);
        assert_eq!(args("f('unterminated", "f"), None);
        assert_eq!(args("f('a') and g()", "f"), None);

        let english = |l: &str| l.eq_ignore_ascii_case("en") || l.to_ascii_lowercase().starts_with("en-");
        assert!(galaxy_has("en-US#", english));
        assert!(galaxy_has("", english));
        assert!(galaxy_has("de-DE#en-US#", english));
        assert!(!galaxy_has("de-DE#", english));
        assert!(galaxy_has("!de-DE#", english));
        assert!(!galaxy_has("!en-US#", english));
        assert!(galaxy_has("32#64#", |a| a == "64"));
        assert!(!galaxy_has("32#", |a| a == "64"));
    }

    /// Little-endian writer for synthetic installer structures.
    #[derive(Default)]
    struct Buf(Vec<u8>);

    impl Buf {
        fn u32(&mut self, v: u32) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        fn u64(&mut self, v: u64) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        fn zeros(&mut self, n: usize) -> &mut Self {
            self.0.resize(self.0.len() + n, 0);
            self
        }
        /// A length-prefixed UTF-16LE string.
        fn ws(&mut self, s: &str) -> &mut Self {
            let units: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
            self.u32(units.len() as u32);
            self.0.extend_from_slice(&units);
            self
        }
    }

    /// A file entry of the fixture: destination, Check, BeforeInstall, AfterInstall, data entry.
    type Entry<'a> = (&'a str, &'a str, String, String, u32);

    /// The decompressed header blocks of an Inno Setup 5.6.2 (u) installer as GOG builds them: one language, then
    /// the file entries and one data entry per stored file, all in a single uncompressed chunk at offset 0.
    fn gog_562_headers(files: &[Entry], data: &[Vec<u8>]) -> (Vec<u8>, Vec<u8>) {
        let mut p = Buf::default();
        p.ws("Outpost 2 - Divided Destiny");
        for _ in 0..25 + 1 + 1 + 2 + 4 {
            p.ws(""); // AppVerName .. CompiledCodeText
        }
        // Languages, messages, permissions, types, components, tasks, dirs, files, data entries, icons, ini, registry,
        // install deletes, uninstall deletes, run, uninstall run.
        for n in [1, 0, 0, 0, 0, 0, 0, files.len() as u32, data.len() as u32, 0, 0, 0, 0, 0, 0, 0] {
            p.u32(n);
        }
        p.zeros(WINVER + 8 + 1 + 28 + 8).u32(1).zeros(3 + 2); // .. SlicesPerDisk .. LanguageDetectionMethod
        p.0.push(3); // CompressMethod: LZMA1 (the fixture's chunk is stored uncompressed anyway)
        p.zeros(2 + 2 + 8 + set_bytes(45));
        // The language: Name .. LicenseText etc., then LanguageID, codepage, font sizes, RightToLeft.
        for s in ["en-US", "English", "", "", "", "", "", "", "", ""] {
            p.ws(s);
        }
        p.zeros(21);
        for (dest, check, before, after, location) in files {
            p.ws("").ws(dest).ws("").ws("").ws("").ws("").ws("").ws(check).ws(after).ws(before);
            p.zeros(WINVER).u32(*location).zeros(4 + 8 + 2 + set_bytes(32) + 1);
        }
        let mut s = Buf::default();
        let chunk_size = 4 + data.iter().map(Vec::len).sum::<usize>() as u64;
        let mut offset = 0;
        for d in data {
            s.u32(0).u32(0).u32(0).u64(offset).u64(d.len() as u64).u64(chunk_size);
            let mut h = sha1_smol::Sha1::new();
            h.update(d);
            s.0.extend_from_slice(&h.digest().bytes());
            s.zeros(16 + 2); // timestamps, versions; flags: not compressed, not filtered
            offset += d.len() as u64;
        }
        (p.0, s.0)
    }

    fn deflate(b: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(b).unwrap();
        z.finish().unwrap()
    }

    fn md5_hex(b: &[u8]) -> String {
        let mut m = Md5::new();
        m.update(b);
        m.digest().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Recreates the GOG Galaxy layout of GOG's Outpost 2 installer (made-up contents): a plain `{app}` file, a
    /// redistributable, a one-part and a two-part game file (the parts stored in reverse order), a file for another
    /// language and `{tmp}` files. Parses it through the 5.6.2 header path and extracts it.
    #[test]
    fn gog_galaxy_installer_extracts() {
        let small = b"a small game file".repeat(10);
        let big: Vec<u8> = (0..30_000u32).map(|i| (i * 7 + i / 300) as u8).collect();
        let (big1, big2) = big.split_at(20_000);
        let english = "check_if_install('en-US#','32#64#','')";
        let german = "check_if_install('de-DE#','32#64#','')";
        let start = |b: &[u8], name: &str, parts: u32| format!("before_install('{}', '{name}', {parts})", md5_hex(b));
        let part = |b: &[u8], stored: &[u8]| format!("after_install('{}', {}, {})", md5_hex(b), stored.len(), b.len());
        let data = vec![
            b"plain".to_vec(),         // 0: {app}\goggame.ico
            deflate(b"dx"),            // 1: __redist part
            deflate(&small),           // 2: game.dat
            deflate(big2),             // 3: second part of sub\big.vol
            deflate(big1),             // 4: first part
            deflate(b"german"),        // 5: de-DE only
            b"background".to_vec(),    // 6: {tmp}
        ];
        let files: Vec<Entry> = vec![
            ("", "", String::new(), String::new(), u32::MAX),
            ("{tmp}\\background.jpg", "", String::new(), String::new(), 6),
            ("{app}\\goggame.ico", "not isWinXP", String::new(), String::new(), 0),
            (
                "{tmp}/11/22\\x",
                "check_if_install_dependency('d14f', False)",
                format!("before_install_dependency('{}', '__redist\\DirectX\\x.cab', 1)", md5_hex(b"dx")),
                format!("after_install_dependency('{}', {}, 2)", md5_hex(b"dx"), data[1].len()),
                1,
            ),
            ("{tmp}/33/44\\x", english, start(&small, "game.dat", 1), part(&small, &data[2]), 2),
            ("{tmp}/55/66\\x", english, start(&big, "sub\\big.vol", 2), part(big1, &data[4]), 4),
            ("{tmp}/77/88\\x", english, String::new(), part(big2, &data[3]), 3),
            ("{tmp}/99/aa\\x", german, start(b"german", "german.txt", 1), part(b"german", &data[5]), 5),
        ];
        let (primary, secondary) = gog_562_headers(&files, &data);
        let mut inst = parse_headers(&primary, &secondary, ver(5, 6, 2, 0)).unwrap();
        assert_eq!(inst.app_name, "Outpost 2 - Divided Destiny");
        let want: Vec<(PathBuf, u64)> = vec![
            ("goggame.ico".into(), 5),
            ("game.dat".into(), small.len() as u64),
            (["sub", "big.vol"].iter().collect(), 30_000),
        ];
        assert_eq!(inst.game_files(), want);

        // The "installer": some loader bytes, then the chunk.
        let dir = std::env::temp_dir().join(format!("op2-inno-galaxy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("setup.exe");
        let mut bytes = vec![0x4d; 1000];
        bytes.extend_from_slice(CHUNK_ID);
        data.iter().for_each(|d| bytes.extend_from_slice(d));
        fs::write(&exe, &bytes).unwrap();
        inst.path = exe;
        inst.data_offset = 1000;
        let out = dir.join("out");
        let mut last = 0;
        let done = inst.extract(&out, &mut |d, _| last = d).unwrap();
        assert_eq!((done.files, done.bytes, last), (3, inst.total_size(), inst.total_size()));
        assert_eq!(fs::read(out.join("goggame.ico")).unwrap(), b"plain");
        assert_eq!(fs::read(out.join("game.dat")).unwrap(), small);
        assert_eq!(fs::read(out.join("sub/big.vol")).unwrap(), big);
        assert!(!out.join("german.txt").exists() && !out.join("__redist").exists());

        // A wrong file MD5 is caught even though every part checks out.
        let i = inst.targets.iter().position(|t| t.parts.len() == 2).unwrap();
        inst.targets[i].md5.as_mut().unwrap()[0] ^= 1;
        let err = inst.extract(&dir.join("out2"), &mut |_, _| {}).unwrap_err().to_string();
        assert!(err.contains("checksum mismatch"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn galaxy_parts_must_add_up() {
        let english = "check_if_install('en-US#','32#64#','')";
        let data = vec![deflate(b"x")];
        let ok = format!("after_install('{}', {}, 1)", md5_hex(b"x"), data[0].len());
        let start = |n: u32| format!("before_install('{}', 'f.dat', {n})", md5_hex(b"x"));
        let headers = |files: &[Entry]| gog_562_headers(files, &data);
        // Two parts announced, one present.
        let (p, s) = headers(&[("{tmp}\\a", english, start(2), ok.clone(), 0)]);
        assert!(parse_headers(&p, &s, ver(5, 6, 2, 0)).unwrap_err().to_string().contains("lacks 1 of its parts"));
        // A stored size that isn't the data entry's.
        let (p, s) = headers(&[("{tmp}\\a", english, start(1), format!("after_install('{}', 99, 1)", md5_hex(b"x")), 0)]);
        assert!(parse_headers(&p, &s, ver(5, 6, 2, 0)).is_err());
        // A path leaving the game folder.
        let evil = format!("before_install('{}', '..\\evil.dll', 1)", md5_hex(b"x"));
        let (p, s) = headers(&[("{tmp}\\a", english, evil, ok.clone(), 0)]);
        assert!(parse_headers(&p, &s, ver(5, 6, 2, 0)).unwrap_err().to_string().contains("unsafe path"));
        let (p, s) = headers(&[("{tmp}\\a", english, start(1), ok, 0)]);
        assert_eq!(parse_headers(&p, &s, ver(5, 6, 2, 0)).unwrap().game_files(), vec![(PathBuf::from("f.dat"), 1)]);
    }
}
