//! Large previews for the Peek panel: pictures, video frames (and muted inline
//! playback), album art, the start of text files, plus facts about the media
//! (dimensions, duration…). Video and audio use the system's ffmpeg/ffprobe when
//! present. Everything slow runs on a background thread.

use eframe::egui;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};

/// Longest edge of the preview picture, in pixels.
pub const PREVIEW_PX: u32 = 720;
const MAX_IMAGE_FILE: u64 = 150 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Video,
    Audio,
    Text,
    Other,
}

pub fn kind_of(mime: &str) -> Kind {
    if crate::thumbs::can_thumbnail(mime) || matches!(mime, "image/tiff" | "image/x-icon" | "image/vnd.microsoft.icon" | "image/x-tga" | "image/x-portable-pixmap") {
        Kind::Image
    } else if mime.starts_with("video/") {
        Kind::Video
    } else if mime.starts_with("audio/") {
        Kind::Audio
    } else if mime.starts_with("text/")
        || ["json", "xml", "javascript", "x-shellscript", "toml", "yaml", "x-desktop", "x-perl", "x-python", "x-ruby", "sql", "x-php"]
            .iter()
            .any(|k| mime.ends_with(k) || mime.contains(&format!("/{k}")))
    {
        Kind::Text
    } else {
        Kind::Other
    }
}

/// What the background thread found out about a file.
#[derive(Default)]
pub struct Preview {
    pub image: Option<egui::ColorImage>,
    /// Extra facts shown under the name ("Dimensions", "1920 × 1080").
    pub facts: Vec<(String, String)>,
    pub text: Option<String>,
    pub duration: Option<f64>,
    /// A note when a preview isn't possible ("Install ffmpeg to preview videos").
    pub note: Option<String>,
}

fn has(bin: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file())).unwrap_or(false)
}

/// Build the preview (slow: call from a background thread).
pub fn load(path: &Path, mime: &str, size: u64) -> Preview {
    load_px(path, mime, size, PREVIEW_PX)
}

/// Like [`load`], with pictures and video frames up to `px` on their longest edge.
pub fn load_px(path: &Path, mime: &str, size: u64, px: u32) -> Preview {
    match kind_of(mime) {
        Kind::Image => image_preview(path, size, px),
        Kind::Video => media_preview(path, true, px),
        Kind::Audio => media_preview(path, false, px),
        Kind::Text => Preview { text: text_head(path), facts: text_facts(path, size), ..Default::default() },
        Kind::Other => Preview::default(),
    }
}

fn image_preview(path: &Path, size: u64, px: u32) -> Preview {
    let mut p = Preview::default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" || ext == "svgz" {
        p.image = crate::icons::load_image_file(path, px);
        p.facts.push(("Format".into(), "Scalable vector".into()));
        return p;
    }
    if let Ok((w, h)) = image::image_dimensions(path) {
        p.facts.push(("Dimensions".into(), format!("{w} × {h} pixels")));
    }
    if size > MAX_IMAGE_FILE {
        p.note = Some("Too large to preview".into());
        return p;
    }
    if let Ok(img) = image::open(path) {
        use image::ColorType as C;
        let (model, bits) = match img.color() {
            C::L8 => ("Grayscale", 8),
            C::La8 => ("Grayscale with transparency", 8),
            C::Rgb8 => ("RGB", 8),
            C::Rgba8 => ("RGB with transparency", 8),
            C::L16 => ("Grayscale", 16),
            C::La16 => ("Grayscale with transparency", 16),
            C::Rgb16 => ("RGB", 16),
            C::Rgba16 => ("RGB with transparency", 16),
            C::Rgb32F => ("RGB", 32),
            C::Rgba32F => ("RGB with transparency", 32),
            _ => ("", 0),
        };
        if bits > 0 {
            p.facts.push(("Color".into(), format!("{model}, {bits}-bit")));
        }
        let mp = img.width() as f64 * img.height() as f64 / 1_000_000.0;
        if mp >= 0.5 {
            p.facts.push(("Megapixels".into(), format!("{:.1} MP", mp)));
        }
        let img = if img.width() > px || img.height() > px { img.thumbnail(px, px) } else { img };
        let rgba = img.to_rgba8();
        p.image = Some(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()));
    }
    p
}

/// The first lines of a text file (None for binary or unreadable files).
fn text_head(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 6000];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.contains(&0) {
        return None;
    }
    // Cut at a character boundary.
    let s = match std::str::from_utf8(&buf) {
        Ok(s) => s.to_string(),
        Err(e) if e.valid_up_to() > n.saturating_sub(4) => String::from_utf8_lossy(&buf[..e.valid_up_to()]).to_string(),
        Err(_) => return None,
    };
    let head = s.lines().take(60).map(|l| l.replace('\t', "    ")).collect::<Vec<_>>().join("\n");
    (!head.trim().is_empty()).then_some(head)
}

/// Lines, words and characters of a text file (only for files up to 4 MB).
fn text_facts(path: &Path, size: u64) -> Vec<(String, String)> {
    if size > 4 * 1024 * 1024 {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let text = String::from_utf8_lossy(&bytes);
    let lines = text.lines().count();
    let words = text.split_whitespace().count();
    let chars = text.chars().count();
    let n = |x: usize| {
        let s = x.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    };
    let enc = if bytes.is_ascii() { "ASCII" } else if std::str::from_utf8(&bytes).is_ok() { "UTF-8" } else { "Other (not UTF-8)" };
    let ends = if bytes.windows(2).any(|w| w == b"\r\n") { "Windows (CRLF)" } else { "Unix (LF)" };
    vec![
        ("Lines".into(), n(lines)),
        ("Words".into(), n(words)),
        ("Characters".into(), n(chars)),
        ("Encoding".into(), enc.into()),
        ("Line endings".into(), ends.into()),
    ]
}

fn fmt_duration(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn duration_text(secs: f64) -> String {
    fmt_duration(secs)
}

/// One value from ffprobe's `key=value` output.
fn probe_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('='))).map(str::trim).filter(|v| !v.is_empty() && *v != "N/A")
}

fn media_preview(path: &Path, video: bool, px: u32) -> Preview {
    let mut p = Preview::default();
    if !has("ffprobe") || !has("ffmpeg") {
        p.note = Some(if video { "Install ffmpeg to preview videos".into() } else { "Install ffmpeg to see details".into() });
        return p;
    }
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration,bit_rate:format_tags=title,artist,album:stream=codec_type,codec_name,width,height,avg_frame_rate,sample_rate,channels", "-of", "default=noprint_wrappers=1"])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let duration = probe_value(&text, "duration").and_then(|d| d.parse::<f64>().ok());
    p.duration = duration;
    let tag = |k: &str| probe_value(&text, &format!("TAG:{k}")).map(str::to_string);
    if !video {
        for (k, label) in [("title", "Title"), ("artist", "Artist"), ("album", "Album")] {
            if let Some(v) = tag(k) {
                p.facts.push((label.into(), v));
            }
        }
    }
    if let Some(d) = duration {
        p.facts.push(("Duration".into(), fmt_duration(d)));
    }
    if video {
        if let (Some(w), Some(h)) = (probe_value(&text, "width"), probe_value(&text, "height")) {
            p.facts.push(("Resolution".into(), format!("{w} × {h}")));
        }
        if let Some(fps) = probe_value(&text, "avg_frame_rate").and_then(|r| {
            let (a, b) = r.split_once('/')?;
            let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
            (b > 0.0 && a > 0.0).then_some(a / b)
        }) {
            p.facts.push(("Frame rate".into(), format!("{} fps", (fps * 100.0).round() / 100.0)));
        }
    } else if let Some(sr) = probe_value(&text, "sample_rate").and_then(|s| s.parse::<f64>().ok()) {
        p.facts.push(("Sample rate".into(), format!("{} kHz", sr / 1000.0)));
    }
    if let Some(c) = probe_value(&text, "codec_name") {
        p.facts.push(("Codec".into(), c.to_string()));
    }
    // A frame a little way in (skips black intros), or the album cover.
    let at = if video { duration.map(|d| (d * 0.1).min(30.0)).unwrap_or(0.0) } else { 0.0 };
    p.image = frame_at(path, at, px);
    if p.image.is_none() && video && at > 0.0 {
        p.image = frame_at(path, 0.0, px);
    }
    p
}

/// One frame as a picture, scaled to at most `px` wide (ffmpeg keeps the shape and rotation).
fn frame_at(path: &Path, secs: f64, px: u32) -> Option<egui::ColorImage> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss", &format!("{secs:.2}"), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &format!("scale='min({px},iw)':-2"), "-f", "image2pipe", "-vcodec", "png", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let img = image::load_from_memory(&out.stdout).ok()?;
    let rgba = img.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}

/// Muted video playback: ffmpeg decodes at real speed into raw frames.
pub struct Player {
    child: Child,
    rx: Receiver<egui::ColorImage>,
    started: Instant,
    start_pos: f64,
    pub ended: bool,
}

impl Player {
    /// Play from `pos` seconds, at the size of the poster frame (`w` × `h`, even numbers).
    pub fn start(path: &PathBuf, pos: f64, w: usize, h: usize, ctx: &egui::Context) -> Option<Player> {
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-nostdin", "-re", "-ss", &format!("{pos:.2}"), "-i"])
            .arg(path)
            .args(["-an", "-sn", "-vf", &format!("scale={w}:{h},fps=30"), "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut out = child.stdout.take()?;
        // Hold at most a couple of frames: the newest one wins.
        let (tx, rx) = sync_channel(2);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; w * h * 4];
            while out.read_exact(&mut buf).is_ok() {
                let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &buf);
                if tx.send(img).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            ctx.request_repaint();
        });
        Some(Player { child, rx, started: Instant::now(), start_pos: pos, ended: false })
    }

    /// The newest decoded frame, if one arrived.
    pub fn frame(&mut self) -> Option<egui::ColorImage> {
        let mut last = None;
        loop {
            match self.rx.try_recv() {
                Ok(f) => last = Some(f),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.ended = true;
                    break;
                }
            }
        }
        last
    }

    pub fn position(&self) -> f64 {
        self.start_pos + self.started.elapsed().as_secs_f64()
    }

    /// How soon the next frame is due.
    pub fn next_frame_in() -> Duration {
        Duration::from_millis(33)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(kind_of("image/png"), Kind::Image);
        assert_eq!(kind_of("video/mp4"), Kind::Video);
        assert_eq!(kind_of("audio/flac"), Kind::Audio);
        assert_eq!(kind_of("text/plain"), Kind::Text);
        assert_eq!(kind_of("application/json"), Kind::Text);
        assert_eq!(kind_of("application/x-shellscript"), Kind::Text);
        assert_eq!(kind_of("application/pdf"), Kind::Other);
        assert_eq!(fmt_duration(75.4), "1:15");
        assert_eq!(fmt_duration(3725.0), "1:02:05");
    }

    /// A tiny generated video gives facts, a poster frame and plays a few frames.
    #[test]
    fn video_preview_and_playback() {
        if !has("ffmpeg") || !has("ffprobe") {
            return;
        }
        let dir = std::env::temp_dir().join(format!("rivet-preview-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("clip.mp4");
        let ok = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=320x180:rate=30:duration=2", "-pix_fmt", "yuv420p"])
            .arg(&f)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return; // no encoder available
        }
        let p = load(&f, "video/mp4", 1);
        let img = p.image.expect("poster frame");
        assert_eq!(img.size, [320, 180]);
        assert!(p.facts.iter().any(|(k, v)| k == "Resolution" && v == "320 × 180"));
        assert!(p.duration.unwrap() > 1.5);
        let ctx = egui::Context::default();
        let mut player = Player::start(&f, 0.0, 320, 180, &ctx).expect("player");
        let t = Instant::now();
        let mut frames = 0;
        while t.elapsed() < Duration::from_secs(5) && !player.ended {
            if player.frame().is_some() {
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(frames > 10, "only {frames} frames");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
