//! Media tools (needs ffmpeg): convert pictures, videos and audio to another format, and
//! resize pictures. Runs in the background with progress, like copying.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::fsops::{self, OpKind, Progress};
use crate::preview::{self, Kind};

/// Ask the system for ffmpeg's version ("7.1.1"), or None if it isn't installed.
fn probe() -> Option<String> {
    // Test hook for checks: behave as if ffmpeg were missing.
    if std::env::var_os("RIVET_NO_FFMPEG").is_some() {
        return None;
    }
    let out = Command::new("ffmpeg").arg("-version").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // "ffmpeg version 7.1.1 Copyright …" → "7.1.1"
    text.lines().next().and_then(|l| l.split_whitespace().nth(2)).map(|s| s.to_string())
}

static CHECKED: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);

/// ffmpeg's version, if ffmpeg is installed. Checked once and then remembered;
/// see [`recheck`] for looking again.
pub fn ffmpeg_version() -> Option<String> {
    let mut c = CHECKED.lock().unwrap();
    if c.is_none() {
        *c = Some((Instant::now(), probe()));
    }
    c.as_ref().and_then(|(_, v)| v.clone())
}

/// Look for ffmpeg again (at most every few seconds), e.g. while Preferences is open,
/// so installing or removing it is noticed without restarting.
pub fn recheck() -> Option<String> {
    {
        let c = CHECKED.lock().unwrap();
        if let Some((at, v)) = c.as_ref()
            && at.elapsed() < Duration::from_secs(4) {
                return v.clone();
            }
    }
    let v = probe();
    *CHECKED.lock().unwrap() = Some((Instant::now(), v.clone()));
    v
}

pub fn available() -> bool {
    ffmpeg_version().is_some()
}

/// Formats offered for each kind of file: (extension, label).
pub fn formats(kind: Kind) -> &'static [(&'static str, &'static str)] {
    match kind {
        Kind::Image => &[("jpg", "JPEG"), ("png", "PNG"), ("webp", "WebP"), ("gif", "GIF"), ("bmp", "BMP"), ("tiff", "TIFF")],
        Kind::Video => &[("mp4", "MP4"), ("webm", "WebM"), ("mkv", "MKV"), ("gif", "GIF"), ("mp3", "MP3 (sound only)")],
        Kind::Audio => &[("mp3", "MP3"), ("ogg", "Ogg Vorbis"), ("flac", "FLAC"), ("wav", "WAV"), ("m4a", "M4A (AAC)")],
        _ => &[],
    }
}

/// The kind shared by all these files, if they all can be converted and are the same kind.
pub fn common_kind(mimes: &[&str]) -> Option<Kind> {
    let mut kind = None;
    for m in mimes {
        let k = preview::kind_of(m);
        if !matches!(k, Kind::Image | Kind::Video | Kind::Audio) {
            return None;
        }
        match kind {
            None => kind = Some(k),
            Some(prev) if prev != k => return None,
            _ => {}
        }
    }
    kind
}

/// What to do with the originals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Write new files next to them.
    Originals,
    /// Replace them (converted originals go to the Trash; resized pictures are overwritten).
    Replace,
}

/// How to resize pictures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Percent(u32),
    /// Fit inside this box, keeping the shape (never enlarges).
    Fit(u32, u32),
}

impl Size {
    /// The new size for a picture of `w` × `h`.
    pub fn apply(self, w: u32, h: u32) -> (u32, u32) {
        match self {
            Size::Percent(p) => (((w as u64 * p as u64) / 100).max(1) as u32, ((h as u64 * p as u64) / 100).max(1) as u32),
            Size::Fit(bw, bh) => {
                let s = (bw as f64 / w.max(1) as f64).min(bh as f64 / h.max(1) as f64).min(1.0);
                (((w as f64 * s).round() as u32).max(1), ((h as f64 * s).round() as u32).max(1))
            }
        }
    }
}

/// Quality 1–100 as ffmpeg arguments for this format.
fn quality_args(ext: &str, quality: u32) -> Vec<String> {
    let q = quality.clamp(1, 100);
    match ext {
        // JPEG: 2 (best) … 31 (worst)
        "jpg" | "jpeg" => vec!["-q:v".into(), (2 + (100 - q) * 29 / 99).to_string()],
        "webp" => vec!["-quality".into(), q.to_string()],
        "mp3" => vec!["-q:a".into(), ((100 - q) * 9 / 99).to_string()],
        "ogg" => vec!["-q:a".into(), (q / 10).to_string()],
        _ => Vec::new(),
    }
}

fn run_ffmpeg(args: &[String], progress: &Progress) -> Result<(), String> {
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
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
            return if status.success() {
                Ok(())
            } else {
                Err(msg.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("ffmpeg failed").to_string())
            };
        }
        if progress.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

/// "photo.png" + "jpg" → "photo.jpg" (or "photo (2).jpg" if that exists).
fn converted_path(src: &Path, ext: &str) -> PathBuf {
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let dir = src.parent().unwrap_or(Path::new("."));
    let want = dir.join(format!("{stem}.{ext}"));
    if want.symlink_metadata().is_err() { want } else { fsops::unique_target(dir, &want) }
}

/// A temporary name next to `p` (same folder, so the final rename is instant).
fn temp_next_to(p: &Path, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    p.with_file_name(format!(".rivet-media-{}-{nanos}.{ext}", std::process::id()))
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Convert one file. Returns the new file's path.
pub fn convert_one(src: &Path, kind: Kind, ext: &str, quality: u32, keep: Keep, progress: &Progress) -> Result<PathBuf, String> {
    let out = converted_path(src, ext);
    let tmp = temp_next_to(&out, ext);
    let mut args: Vec<String> = vec!["-i".into(), src.to_string_lossy().to_string()];
    if kind == Kind::Image {
        args.extend(["-frames:v".into(), "1".into(), "-update".into(), "1".into()]);
    }
    if ext == "mp3" && kind == Kind::Video {
        args.push("-vn".into());
    }
    if ext == "gif" && kind == Kind::Video {
        // A reasonable GIF: 12 fps, at most 640 wide.
        args.extend(["-vf".into(), "fps=12,scale='min(640,iw)':-2:flags=lanczos".into()]);
    }
    args.extend(quality_args(ext, quality));
    args.push(tmp.to_string_lossy().to_string());
    if let Err(e) = run_ffmpeg(&args, progress) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    if keep == Keep::Replace && out != src {
        // The original goes to the Trash, so nothing is lost by accident.
        fsops::trash(src).map_err(|e| format!("Converted, but could not move the original to the Trash: {e}"))?;
    }
    Ok(out)
}

/// Resize one picture. Returns the resulting file's path.
pub fn resize_one(src: &Path, size: Size, keep: Keep, progress: &Progress) -> Result<PathBuf, String> {
    let (w, h) = image::image_dimensions(src).map_err(|e| format!("Not a readable picture: {e}"))?;
    let (nw, nh) = size.apply(w, h);
    let ext = ext_of(src);
    let ext = if ext.is_empty() { "png".to_string() } else { ext };
    let out = match keep {
        Keep::Replace => src.to_path_buf(),
        Keep::Originals => {
            let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let want = src.with_file_name(format!("{stem} ({nw}×{nh}).{ext}"));
            if want.symlink_metadata().is_err() { want } else { fsops::unique_target(src.parent().unwrap_or(Path::new(".")), &want) }
        }
    };
    let tmp = temp_next_to(&out, &ext);
    let mut args: Vec<String> = vec!["-i".into(), src.to_string_lossy().to_string(), "-vf".into(), format!("scale={nw}:{nh}:flags=lanczos")];
    args.extend(["-frames:v".into(), "1".into(), "-update".into(), "1".into()]);
    args.extend(quality_args(&ext, 92));
    args.push(tmp.to_string_lossy().to_string());
    if let Err(e) = run_ffmpeg(&args, progress) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Convert many files in the background.
pub fn spawn_convert(files: Vec<PathBuf>, kind: Kind, ext: String, quality: u32, keep: Keep) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(OpKind::Convert, files.len(), 0));
    let pp = p.clone();
    std::thread::spawn(move || {
        for f in files {
            if pp.cancel.load(Ordering::Relaxed) {
                break;
            }
            *pp.current.lock().unwrap() = name_of(&f);
            match convert_one(&f, kind, &ext, quality, keep, &pp) {
                Ok(out) => {
                    pp.created.lock().unwrap().push(out.clone());
                    // Only new files kept next to the originals can be undone (moved to the Trash).
                    if keep == Keep::Originals {
                        pp.done.lock().unwrap().push((f, out));
                    }
                }
                Err(e) if e == "Cancelled" => {}
                Err(e) => pp.errors.lock().unwrap().push(format!("{}: {e}", name_of(&f))),
            }
            pp.done_items.fetch_add(1, Ordering::Relaxed);
        }
        pp.finished.store(true, Ordering::Release);
    });
    p
}

/// Resize many pictures in the background.
pub fn spawn_resize(files: Vec<PathBuf>, size: Size, keep: Keep) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(OpKind::Resize, files.len(), 0));
    let pp = p.clone();
    std::thread::spawn(move || {
        for f in files {
            if pp.cancel.load(Ordering::Relaxed) {
                break;
            }
            *pp.current.lock().unwrap() = name_of(&f);
            match resize_one(&f, size, keep, &pp) {
                Ok(out) => {
                    pp.created.lock().unwrap().push(out.clone());
                    if keep == Keep::Originals {
                        pp.done.lock().unwrap().push((f, out));
                    }
                }
                Err(e) if e == "Cancelled" => {}
                Err(e) => pp.errors.lock().unwrap().push(format!("{}: {e}", name_of(&f))),
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

    #[test]
    fn sizes() {
        assert_eq!(Size::Percent(50).apply(1200, 800), (600, 400));
        assert_eq!(Size::Fit(800, 800).apply(1600, 1000), (800, 500));
        assert_eq!(Size::Fit(4000, 4000).apply(1600, 1000), (1600, 1000)); // never enlarges
        assert_eq!(common_kind(&["image/png", "image/jpeg"]), Some(Kind::Image));
        assert_eq!(common_kind(&["image/png", "video/mp4"]), None);
        assert_eq!(common_kind(&["text/plain"]), None);
    }

    #[test]
    fn convert_and_resize() {
        if !available() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("rivet-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("pic.png");
        let img = image::RgbImage::from_fn(64, 40, |x, y| image::Rgb([x as u8 * 4, y as u8 * 6, 128]));
        img.save(&src).unwrap();
        let wait = |p: &Progress| {
            while !p.finished.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        // Convert, keeping the original.
        let p = spawn_convert(vec![src.clone()], Kind::Image, "jpg".into(), 90, Keep::Originals);
        wait(&p);
        assert!(p.errors.lock().unwrap().is_empty(), "{:?}", p.errors.lock().unwrap());
        assert!(src.exists());
        assert_eq!(image::image_dimensions(dir.join("pic.jpg")).unwrap(), (64, 40));
        // Resize to half, as a new file.
        let p = spawn_resize(vec![src.clone()], Size::Percent(50), Keep::Originals);
        wait(&p);
        assert!(p.errors.lock().unwrap().is_empty(), "{:?}", p.errors.lock().unwrap());
        assert_eq!(image::image_dimensions(dir.join("pic (32×20).png")).unwrap(), (32, 20));
        // Resize in place.
        let p = spawn_resize(vec![src.clone()], Size::Fit(16, 16), Keep::Replace);
        wait(&p);
        assert_eq!(image::image_dimensions(&src).unwrap(), (16, 10));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
