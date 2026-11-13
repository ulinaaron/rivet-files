//! Compress and extract, using the archive tools the system already has
//! (zip/unzip, tar with gzip/xz/zstd, 7z). Runs in the background like copying,
//! shows progress in the status bar, and can be undone.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::fsops::{self, OpKind, Progress};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Zip,
    TarGz,
    TarXz,
    TarZst,
    SevenZ,
}

impl Format {
    pub const ALL: [Format; 5] = [Format::Zip, Format::TarGz, Format::TarXz, Format::TarZst, Format::SevenZ];

    pub fn ext(self) -> &'static str {
        match self {
            Format::Zip => ".zip",
            Format::TarGz => ".tar.gz",
            Format::TarXz => ".tar.xz",
            Format::TarZst => ".tar.zst",
            Format::SevenZ => ".7z",
        }
    }

    /// A short hint shown under the choice.
    pub fn hint(self) -> &'static str {
        match self {
            Format::Zip => "Opens almost anywhere, including Windows and macOS",
            Format::TarGz => "The common Linux format, fast",
            Format::TarXz => "Smaller files, slower to make",
            Format::TarZst => "Small and very fast",
            Format::SevenZ => "Smallest files, needs 7-Zip to open",
        }
    }

    pub fn available(self) -> bool {
        match self {
            Format::Zip => has("zip"),
            Format::TarGz => has("tar") && has("gzip"),
            Format::TarXz => has("tar") && has("xz"),
            Format::TarZst => has("tar") && has("zstd"),
            Format::SevenZ => seven_zip().is_some(),
        }
    }

    pub fn from_id(id: &str) -> Format {
        Format::ALL.into_iter().find(|f| f.ext().trim_start_matches('.') == id).unwrap_or(Format::Zip)
    }
}

fn has(bin: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file())).unwrap_or(false)
}

fn seven_zip() -> Option<&'static str> {
    ["7z", "7zz", "7za"].into_iter().find(|b| has(b))
}

/// The archive endings Rivet can open, longest first so ".tar.gz" wins over ".gz".
const ARCHIVE_EXTS: &[&str] = &[
    ".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tar.lz", ".tar.lzma", ".tar.z", ".tgz", ".txz", ".tbz", ".tbz2", ".tzst", ".tar",
    ".zip", ".jar", ".7z", ".rar", ".gz", ".xz", ".bz2", ".zst", ".lz", ".lzma", ".iso", ".cab", ".deb", ".rpm",
];

/// The archive ending of a file name, if it is an archive.
fn archive_ext(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    ARCHIVE_EXTS.iter().copied().find(|e| lower.ends_with(e) && lower.len() > e.len())
}

pub fn is_archive(path: &Path) -> bool {
    path.is_file() && path.file_name().and_then(|n| n.to_str()).and_then(archive_ext).is_some()
}

/// "photos.tar.gz" → "photos".
pub fn stem(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match archive_ext(&name) {
        Some(e) => name[..name.len() - e.len()].to_string(),
        None => name,
    }
}

/// A suggested archive name (without ending) for these items.
pub fn suggest_name(paths: &[PathBuf]) -> String {
    match paths {
        [one] => {
            let name = one.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if one.is_dir() {
                name
            } else {
                one.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or(name)
            }
        }
        _ => paths
            .first()
            .and_then(|p| p.parent())
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Archive".into()),
    }
}

/// Run a tool, stopping it if the operation is cancelled. Errors carry the tool's message.
fn run(mut cmd: Command, progress: &Progress) -> Result<(), String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the archive tool: {e}"))?;
    let stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            use std::io::Read;
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let msg = reader.join().unwrap_or_default();
            if status.success() {
                return Ok(());
            }
            let line = msg.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("The archive tool failed").to_string();
            return Err(line);
        }
        if progress.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Make `archive` from `sources` (all in one folder).
pub fn compress(sources: &[PathBuf], archive: &Path, format: Format, progress: &Progress) -> Result<(), String> {
    let dir = sources.first().and_then(|p| p.parent()).ok_or("Nothing to compress")?;
    let names: Vec<String> = sources.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).collect();
    let mut cmd;
    match format {
        Format::Zip => {
            cmd = Command::new("zip");
            cmd.args(["-r", "-q", "-y"]).arg(archive).arg("--").args(&names);
        }
        Format::SevenZ => {
            cmd = Command::new(seven_zip().ok_or("7-Zip is not installed")?);
            cmd.args(["a", "-bd", "-y"]).arg(archive).arg("--").args(&names);
        }
        Format::TarGz | Format::TarXz | Format::TarZst => {
            cmd = Command::new("tar");
            let flag = match format {
                Format::TarGz => "--gzip",
                Format::TarXz => "--xz",
                _ => "--zstd",
            };
            cmd.args([flag, "-cf"]).arg(archive).arg("--").args(&names);
        }
    }
    cmd.current_dir(dir);
    let r = run(cmd, progress);
    if r.is_err() {
        let _ = std::fs::remove_file(archive);
    }
    r
}

/// Unpack `archive` into `into`. Like most file managers: an archive with a single
/// top-level item puts just that item there; anything else gets its own folder
/// named after the archive. Returns what was made.
pub fn extract(archive: &Path, into: &Path, progress: &Progress) -> Result<PathBuf, String> {
    let name = archive.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let ext = archive_ext(&name).ok_or("This is not an archive Rivet knows")?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let tmp = into.join(format!(".rivet-extract-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("Could not write here: {e}"))?;
    let result = (|| {
        let single_stream = matches!(ext, ".gz" | ".xz" | ".bz2" | ".zst" | ".lz" | ".lzma");
        if single_stream {
            // A single compressed file (not a tar): decompress it to a file.
            let tool = match ext {
                ".gz" => "gzip",
                ".xz" | ".lzma" => "xz",
                ".bz2" => "bzip2",
                ".zst" => "zstd",
                _ => "lzip",
            };
            let out = tmp.join(stem(archive));
            let file = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            let mut cmd = Command::new(tool);
            cmd.arg("-dc").arg(archive).stdout(file).stdin(Stdio::null()).stderr(Stdio::piped());
            let status = cmd.status().map_err(|e| format!("Could not start {tool}: {e}"))?;
            return if status.success() { Ok(()) } else { Err(format!("{tool} could not read the file")) };
        }
        let is_tar = ext.starts_with(".tar") || matches!(ext, ".tgz" | ".txz" | ".tbz" | ".tbz2" | ".tzst");
        let mut cmd;
        if is_tar && has("tar") {
            cmd = Command::new("tar");
            cmd.arg("-xf").arg(archive).arg("-C").arg(&tmp);
        } else if matches!(ext, ".zip" | ".jar") && has("unzip") {
            cmd = Command::new("unzip");
            cmd.args(["-q", "-o"]).arg(archive).arg("-d").arg(&tmp);
        } else if let Some(z) = seven_zip() {
            cmd = Command::new(z);
            cmd.args(["x", "-bd", "-y"]).arg(format!("-o{}", tmp.display())).arg(archive);
        } else if has("bsdtar") {
            cmd = Command::new("bsdtar");
            cmd.arg("-xf").arg(archive).arg("-C").arg(&tmp);
        } else {
            return Err("No tool to open this kind of archive is installed (try installing 7-Zip)".into());
        }
        run(cmd, progress)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let items: Vec<PathBuf> = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    let made = if items.len() == 1 {
        let only = &items[0];
        let target = fsops::unique_target(into, only);
        std::fs::rename(only, &target).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir(&tmp);
        target
    } else {
        let target = fsops::unique_target(into, &into.join(stem(archive)));
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
        target
    };
    Ok(made)
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Compress in the background. `archive` is the full path of the archive to make.
pub fn spawn_compress(sources: Vec<PathBuf>, archive: PathBuf, format: Format) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(OpKind::Compress, 1, 0));
    let pp = p.clone();
    std::thread::spawn(move || {
        *pp.current.lock().unwrap() = name_of(&archive);
        match compress(&sources, &archive, format, &pp) {
            Ok(()) => {
                pp.created.lock().unwrap().push(archive.clone());
                pp.done.lock().unwrap().push((sources[0].clone(), archive));
            }
            Err(e) if e == "Cancelled" => {}
            Err(e) => pp.errors.lock().unwrap().push(format!("{}: {e}", name_of(&archive))),
        }
        pp.done_items.fetch_add(1, Ordering::Relaxed);
        pp.finished.store(true, Ordering::Release);
    });
    p
}

/// Extract each archive into `into` in the background.
pub fn spawn_extract(archives: Vec<PathBuf>, into: PathBuf) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(OpKind::Extract, archives.len(), 0));
    let pp = p.clone();
    std::thread::spawn(move || {
        for a in archives {
            if pp.cancel.load(Ordering::Relaxed) {
                break;
            }
            *pp.current.lock().unwrap() = name_of(&a);
            match extract(&a, &into, &pp) {
                Ok(made) => {
                    pp.created.lock().unwrap().push(made.clone());
                    pp.done.lock().unwrap().push((a, made));
                }
                Err(e) if e == "Cancelled" => {}
                Err(e) => pp.errors.lock().unwrap().push(format!("{}: {e}", name_of(&a))),
            }
            pp.done_items.fetch_add(1, Ordering::Relaxed);
        }
        pp.finished.store(true, Ordering::Release);
    });
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(p: &Progress) {
        while !p.finished.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn names() {
        assert_eq!(stem(Path::new("/x/photos.tar.gz")), "photos");
        assert_eq!(stem(Path::new("/x/a.b.zip")), "a.b");
        assert!(archive_ext("x.TGZ").is_some());
        assert!(archive_ext("notes.txt").is_none());
        assert!(archive_ext(".zip").is_none());
        assert_eq!(suggest_name(&[PathBuf::from("/x/report.pdf")]), "report");
    }

    /// Compress then extract every available format; a folder of loose files
    /// comes back in its own folder, a single folder comes back as itself.
    #[test]
    fn round_trip() {
        let base = std::env::temp_dir().join(format!("rivet-archive-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        std::fs::create_dir_all(src.join("Folder/inner")).unwrap();
        std::fs::write(src.join("Folder/inner/a.txt"), "hello").unwrap();
        std::fs::write(src.join("b.txt"), "world").unwrap();
        for f in Format::ALL.into_iter().filter(|f| f.available()) {
            let out = base.join(format!("out-{}", f.ext().trim_matches('.').replace('.', "-")));
            std::fs::create_dir_all(&out).unwrap();
            // one folder
            let arc = out.join(format!("Folder{}", f.ext()));
            let p = spawn_compress(vec![src.join("Folder")], arc.clone(), f);
            wait(&p);
            assert!(p.errors.lock().unwrap().is_empty(), "{f:?}: {:?}", p.errors.lock().unwrap());
            let x = base.join(format!("x1-{}", f.ext().trim_matches('.').replace('.', "-")));
            std::fs::create_dir_all(&x).unwrap();
            let e = spawn_extract(vec![arc], x.clone());
            wait(&e);
            assert!(e.errors.lock().unwrap().is_empty(), "{f:?}: {:?}", e.errors.lock().unwrap());
            assert_eq!(std::fs::read_to_string(x.join("Folder/inner/a.txt")).unwrap(), "hello", "{f:?}");
            // two items → a folder named after the archive
            let arc2 = out.join(format!("Both{}", f.ext()));
            let p = spawn_compress(vec![src.join("Folder"), src.join("b.txt")], arc2.clone(), f);
            wait(&p);
            let e = spawn_extract(vec![arc2], x.clone());
            wait(&e);
            assert_eq!(std::fs::read_to_string(x.join("Both/b.txt")).unwrap(), "world", "{f:?}");
            // no temporary folders left behind
            assert!(std::fs::read_dir(&x).unwrap().flatten().all(|d| !d.file_name().to_string_lossy().starts_with(".rivet-extract")));
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
