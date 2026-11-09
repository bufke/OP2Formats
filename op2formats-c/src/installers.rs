//! The installers: GOG's offline installer (Inno Setup, feature `inno`), the OPU update's installer (NSIS, feature
//! `opu`), and fetching OPU's installer (feature `fetch-opu`). Without its feature a function stays in the library
//! and fails with `OP2F_STATUS_UNSUPPORTED`.

use crate::*;
use std::ffi::{CString, c_char, c_void};

// ---- GOG's installer (Inno Setup) ----

/// GOG's offline installer (`setup_outpost_2_*.exe`, plus any `-N.bin` parts next to it), opened: its header read,
/// nothing extracted yet.
pub struct Op2fInnoInstaller {
    #[cfg(feature = "inno")]
    pub(crate) inst: op2_formats::inno::Installer,
}

/// What an extraction wrote.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Op2fExtracted {
    pub files: usize,
    pub bytes: u64,
}

/// Opens an installer by path. Errors say what's wrong in words a player can act on.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_open(path: *const c_char, out: *mut *mut Op2fInnoInstaller) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        #[cfg(feature = "inno")]
        {
            let inst = op2_formats::inno::Installer::open(&path).map_err(lib_err)?;
            unsafe { put(out, Op2fInnoInstaller { inst }) }
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (path, out);
            Err(without_feature("inno"))
        }
    })
}

/// # Safety
/// `inst` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_free(inst: *mut Op2fInnoInstaller) {
    unsafe { free(inst) }
}

/// A text of an installer's header.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fInnoText {
    /// The data version line, e.g. "Inno Setup Setup Data (5.6.2) (u)".
    Version,
    /// The product it installs ("Outpost 2" for GOG's).
    AppName,
}

/// # Safety
/// `inst` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_text(
    inst: *const Op2fInnoInstaller,
    field: Op2fInnoText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let inst = unsafe { obj(inst, "inst") }?;
        #[cfg(feature = "inno")]
        {
            let text = match field {
                Op2fInnoText::Version => &inst.inst.version_text,
                Op2fInnoText::AppName => &inst.inst.app_name,
            };
            unsafe { copy_str(text, buf, cap, len) }
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (inst, field, buf, cap, len);
            Err(without_feature("inno"))
        }
    })
}

/// The files the installer puts in the game folder: paths relative to it, with `/` separators.
///
/// # Safety
/// `inst` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_files(inst: *const Op2fInnoInstaller, out: *mut *mut Op2fStringList) -> Op2fStatus {
    guard(|| {
        let inst = unsafe { obj(inst, "inst") }?;
        #[cfg(feature = "inno")]
        {
            let files = inst.inst.game_files().iter().map(|(p, _)| p.to_string_lossy().replace('\\', "/")).collect();
            unsafe { put(out, Op2fStringList(files)) }
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (inst, out);
            Err(without_feature("inno"))
        }
    })
}

/// File `index`'s size in bytes (in `op2f_inno_files`' order).
///
/// # Safety
/// `inst` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_file_size(inst: *const Op2fInnoInstaller, index: usize, out: *mut u64) -> Op2fStatus {
    guard(|| {
        let inst = unsafe { obj(inst, "inst") }?;
        #[cfg(feature = "inno")]
        {
            let files = inst.inst.game_files();
            unsafe { put_value(out, self::index(&files, index, "file")?.1) }
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (inst, index, out);
            Err(without_feature("inno"))
        }
    })
}

/// Total bytes an extraction writes.
///
/// # Safety
/// `inst` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_total_size(inst: *const Op2fInnoInstaller) -> u64 {
    #[cfg(feature = "inno")]
    {
        unsafe { inst.as_ref() }.map_or(0, |i| i.inst.total_size())
    }
    #[cfg(not(feature = "inno"))]
    {
        let _ = inst;
        0
    }
}

/// Extracts the game folder into `dir` (created if needed), checking every file against the installer's checksums.
/// `progress` (may be NULL) gets stage 0 and (bytes written, total) as it goes; `out` (may be NULL) what was written.
///
/// # Safety
/// `inst` is a live handle; `dir` a string; `progress` NULL or a function taking `user`; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_extract(
    inst: *const Op2fInnoInstaller,
    dir: *const c_char,
    progress: Op2fProgress,
    user: *mut c_void,
    out: *mut Op2fExtracted,
) -> Op2fStatus {
    guard(|| {
        let inst = unsafe { obj(inst, "inst") }?;
        let dir = unsafe { path_arg(dir, "dir") }?;
        #[cfg(feature = "inno")]
        {
            let done = inst.inst.extract(&dir, &mut |d, t| report(progress, user, 0, d, t)).map_err(lib_err)?;
            if !out.is_null() {
                unsafe { out.write(Op2fExtracted { files: done.files, bytes: done.bytes }) };
            }
            Ok(())
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (inst, dir, progress, user, out);
            Err(without_feature("inno"))
        }
    })
}

/// Extracts GOG's installer at `installer` into `target` and checks the result, as op2-rs's setup does: through
/// `<target>.partial`, renamed into place only when complete. `target` must be empty or absent, or be the data
/// folder's extract folder (`op2f_data_dir`), which is replaced. `progress` gets stage 0 and bytes.
///
/// # Safety
/// `installer` and `target` are strings; `progress` NULL or a function taking `user`; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_inno_extract_game(
    installer: *const c_char,
    target: *const c_char,
    progress: Op2fProgress,
    user: *mut c_void,
    out: *mut *mut Op2fInstallCheck,
) -> Op2fStatus {
    guard(|| {
        let installer = unsafe { path_arg(installer, "installer") }?;
        let target = unsafe { path_arg(target, "target") }?;
        #[cfg(feature = "inno")]
        {
            let check = op2_formats::install::extract_installer(&installer, &target, &mut |d, t| report(progress, user, 0, d, t))
                .map_err(lib_err)?;
            unsafe { put(out, Op2fInstallCheck(check)) }
        }
        #[cfg(not(feature = "inno"))]
        {
            let _ = (installer, target, progress, user, out);
            Err(without_feature("inno"))
        }
    })
}

// ---- The OPU update ----

/// Where an OPU install or download is (the `stage` given to `Op2fProgress`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpuStage {
    /// Downloading the installer: bytes so far and in total.
    Download = 0,
    /// Checking and unpacking the installer.
    Read = 1,
    /// Writing files: files so far and in total.
    Install = 2,
}

#[cfg(feature = "opu")]
fn stage(s: op2_formats::opu::Stage) -> u32 {
    use op2_formats::opu::Stage;
    (match s {
        Stage::Download => Op2fOpuStage::Download,
        Stage::Read => Op2fOpuStage::Read,
        Stage::Install => Op2fOpuStage::Install,
    }) as u32
}

/// A fact about the OPU release op2-formats installs.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpuText {
    /// "1.4.1".
    Version,
    /// The official download page.
    DownloadPage,
    /// The installer's URL.
    PackageUrl,
    /// The installer's file name.
    PackageName,
    /// The installer's SHA-256, in hex.
    PackageSha256,
    /// The credit OPU's license asks for.
    Credit,
}

/// A fact about the OPU release, as a static string.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_opu_text(field: Op2fOpuText) -> *const c_char {
    use op2_formats::opu;
    static TEXTS: std::sync::OnceLock<[CString; 6]> = std::sync::OnceLock::new();
    let texts = TEXTS.get_or_init(|| {
        [opu::VERSION, opu::DOWNLOAD_PAGE, opu::PACKAGE_URL, opu::PACKAGE_NAME, opu::PACKAGE_SHA256, opu::CREDIT]
            .map(|s| CString::new(s).expect("no NUL in OPU's facts"))
    });
    texts[field as usize].as_ptr()
}

/// The size of OPU's installer, in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_opu_package_size() -> u64 {
    op2_formats::opu::PACKAGE_SIZE
}

/// Checks that bytes are OPU's official installer (size and SHA-256).
///
/// # Safety
/// `data` is `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_verify_package(data: *const u8, len: usize) -> Op2fStatus {
    guard(|| {
        let data = unsafe { bytes_arg(data, len) }?;
        #[cfg(feature = "opu")]
        {
            op2_formats::opu::verify_package(data).map_err(lib_err)
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = data;
            Err(without_feature("opu"))
        }
    })
}

/// An OPU folder in or under `dir` (one already unpacked by the player); `OP2F_STATUS_NOT_FOUND` when there's none.
///
/// # Safety
/// `dir` is a string; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_find_folder(dir: *const c_char, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(dir, "dir") }?;
        #[cfg(feature = "opu")]
        {
            let found = op2_formats::opu::find_opu_folder(&dir)
                .ok_or_else(|| fail(Op2fStatus::NotFound, format!("no OPU folder in {}", dir.display())))?;
            unsafe { copy_str(&found.to_string_lossy(), buf, cap, len) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (dir, buf, cap, len);
            Err(without_feature("opu"))
        }
    })
}

/// What an OPU install did.
pub struct Op2fOpuInstalled {
    #[cfg(feature = "opu")]
    pub(crate) installed: op2_formats::opu::Installed,
}

/// Installs OPU from its installer (`package`, or an unpacked OPU folder) for the game at `game_dir` into `target`
/// (e.g. `op2f_data_dir(OP2F_DATA_DIR_OPU)`). The game folder isn't changed. `progress` gets an `Op2fOpuStage`.
///
/// # Safety
/// The paths are strings; `progress` NULL or a function taking `user`; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_install(
    package: *const c_char,
    game_dir: *const c_char,
    target: *const c_char,
    progress: Op2fProgress,
    user: *mut c_void,
    out: *mut *mut Op2fOpuInstalled,
) -> Op2fStatus {
    guard(|| {
        let package = unsafe { path_arg(package, "package") }?;
        let game_dir = unsafe { path_arg(game_dir, "game_dir") }?;
        let target = unsafe { path_arg(target, "target") }?;
        #[cfg(feature = "opu")]
        {
            let installed = op2_formats::opu::install(&package, &game_dir, &target, &mut |s, d, t| report(progress, user, stage(s), d, t))
                .map_err(lib_err)?;
            unsafe { put(out, Op2fOpuInstalled { installed }) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (package, game_dir, target, progress, user, out);
            Err(without_feature("opu"))
        }
    })
}

/// # Safety
/// `installed` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_installed_free(installed: *mut Op2fOpuInstalled) {
    unsafe { free(installed) }
}

/// Where OPU was installed and, when `files` isn't NULL, how many files were written.
///
/// # Safety
/// `installed` is a live handle; `files` NULL or writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_installed_dir(
    installed: *const Op2fOpuInstalled,
    files: *mut usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let installed = unsafe { obj(installed, "installed") }?;
        #[cfg(feature = "opu")]
        {
            let i = &installed.installed;
            if !files.is_null() {
                unsafe { files.write(i.files) };
            }
            unsafe { copy_str(&i.dir.to_string_lossy(), buf, cap, len) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (installed, files, buf, cap, len);
            Err(without_feature("opu"))
        }
    })
}

/// Things worth telling the player about the install (it still works).
///
/// # Safety
/// `installed` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_installed_warnings(installed: *const Op2fOpuInstalled, out: *mut *mut Op2fStringList) -> Op2fStatus {
    guard(|| {
        let installed = unsafe { obj(installed, "installed") }?;
        #[cfg(feature = "opu")]
        {
            unsafe { put(out, Op2fStringList(installed.installed.warnings.clone())) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (installed, out);
            Err(without_feature("opu"))
        }
    })
}

/// A digest of every file under `dir` (names and contents), to tell whether an OPU install changed.
///
/// # Safety
/// `dir` is a string; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_tree_digest(dir: *const c_char, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(dir, "dir") }?;
        #[cfg(feature = "opu")]
        {
            unsafe { copy_str(&op2_formats::opu::tree_digest(&dir).map_err(lib_err)?, buf, cap, len) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (dir, buf, cap, len);
            Err(without_feature("opu"))
        }
    })
}

/// Downloads OPU's official installer from outpost2.net into `dir`, checking its SHA-256, and gives its path. Only
/// with the `fetch-opu` feature. `progress` gets `OP2F_OPU_STAGE_DOWNLOAD` and bytes.
///
/// # Safety
/// `dir` is a string; `progress` NULL or a function taking `user`; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opu_download(
    dir: *const c_char,
    progress: Op2fProgress,
    user: *mut c_void,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let dir = unsafe { path_arg(dir, "dir") }?;
        #[cfg(feature = "fetch-opu")]
        {
            let path = op2_formats::opu::download(&dir, &mut |s, d, t| report(progress, user, stage(s), d, t)).map_err(lib_err)?;
            unsafe { copy_str(&path.to_string_lossy(), buf, cap, len) }
        }
        #[cfg(not(feature = "fetch-opu"))]
        {
            let _ = (dir, progress, user, buf, cap, len);
            Err(without_feature("fetch-opu"))
        }
    })
}

// ---- NSIS installers ----

/// A Nullsoft (NSIS 3, LZMA) installer's files, as OPU's installer is built.
pub struct Op2fNsis {
    #[cfg(feature = "opu")]
    pub(crate) nsis: op2_formats::nsis::Nsis,
}

/// Reads an NSIS installer from memory.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_nsis_parse(data: *const u8, len: usize, out: *mut *mut Op2fNsis) -> Op2fStatus {
    guard(|| {
        let data = unsafe { bytes_arg(data, len) }?;
        #[cfg(feature = "opu")]
        {
            let nsis = op2_formats::nsis::Nsis::parse(data).map_err(lib_err)?;
            unsafe { put(out, Op2fNsis { nsis }) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (data, out);
            Err(without_feature("opu"))
        }
    })
}

/// # Safety
/// `nsis` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_nsis_free(nsis: *mut Op2fNsis) {
    unsafe { free(nsis) }
}

/// Where each file the install script extracts goes, in script order, with variables as `$NAME` and `\`
/// separators (e.g. `$INSTDIR\OPU\README.md`).
///
/// # Safety
/// `nsis` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_nsis_files(nsis: *const Op2fNsis, out: *mut *mut Op2fStringList) -> Op2fStatus {
    guard(|| {
        let nsis = unsafe { obj(nsis, "nsis") }?;
        #[cfg(feature = "opu")]
        {
            unsafe { put(out, Op2fStringList(nsis.nsis.files().iter().map(|f| f.path.clone()).collect())) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (nsis, out);
            Err(without_feature("opu"))
        }
    })
}

/// The contents of file `index` (in `op2f_nsis_files`' order).
///
/// # Safety
/// `nsis` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_nsis_read(nsis: *const Op2fNsis, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let nsis = unsafe { obj(nsis, "nsis") }?;
        #[cfg(feature = "opu")]
        {
            let f = self::index(nsis.nsis.files(), index, "file")?;
            unsafe { put(out, Op2fBytes(nsis.nsis.read(f).map_err(lib_err)?)) }
        }
        #[cfg(not(feature = "opu"))]
        {
            let _ = (nsis, index, out);
            Err(without_feature("opu"))
        }
    })
}
