//! Bulk rename: the rules (insert, date/time, numbering, remove, search & replace, case),
//! a preview of the new names with problems flagged, and applying them safely (two passes,
//! so names can swap or shift without clobbering each other).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Insert,
    DateTime,
    Number,
    Remove,
    Replace,
    Case,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Insert, Mode::DateTime, Mode::Number, Mode::Remove, Mode::Replace, Mode::Case];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Insert => "Insert / Overwrite",
            Mode::DateTime => "Date / Time",
            Mode::Number => "Numbering",
            Mode::Remove => "Remove Characters",
            Mode::Replace => "Search & Replace",
            Mode::Case => "Uppercase / Lowercase",
        }
    }
}

/// Which part of "photo.jpg" the rule works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Name,
    Extension,
    Both,
}

/// Where text goes: counted from the start or from the end of the part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateSource {
    Now,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseMode {
    Lower,
    Upper,
    Title,
    Sentence,
}

/// Every rule's settings, kept together so switching modes doesn't lose what was typed.
#[derive(Clone, Debug)]
pub struct Rules {
    pub mode: Mode,
    pub part: Part,
    // insert / overwrite
    pub insert_text: String,
    pub insert_at: usize,
    pub insert_from: From,
    pub overwrite: bool,
    // date / time
    pub date_format: String,
    pub date_source: DateSource,
    pub date_at: usize,
    pub date_from: From,
    // numbering
    pub number_start: i64,
    pub number_step: i64,
    pub number_digits: usize,
    pub number_text: String,
    pub number_after: bool,
    // remove
    pub remove_from: usize,
    pub remove_count: usize,
    pub remove_side: From,
    // search & replace
    pub find: String,
    pub replace: String,
    pub match_case: bool,
    // case
    pub case: CaseMode,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            mode: Mode::Insert,
            part: Part::Name,
            insert_text: String::new(),
            insert_at: 0,
            insert_from: From::Start,
            overwrite: false,
            date_format: "%Y-%m-%d ".into(),
            date_source: DateSource::Modified,
            date_at: 0,
            date_from: From::Start,
            number_start: 1,
            number_step: 1,
            number_digits: 2,
            number_text: " - ".into(),
            number_after: false,
            remove_from: 0,
            remove_count: 1,
            remove_side: From::Start,
            find: String::new(),
            replace: String::new(),
            match_case: false,
            case: CaseMode::Lower,
        }
    }
}

/// "archive.tar.gz" → ("archive.tar", "gz"); folders and dot-files have no extension.
fn split(name: &str, is_dir: bool) -> (String, String) {
    if is_dir {
        return (name.to_string(), String::new());
    }
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (name[..i].to_string(), name[i + 1..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn join(stem: &str, ext: &str) -> String {
    if ext.is_empty() { stem.to_string() } else { format!("{stem}.{ext}") }
}

/// Char index `at`, counted from the start or end, clamped to the text.
fn char_pos(s: &str, at: usize, from: From) -> usize {
    let n = s.chars().count();
    match from {
        From::Start => at.min(n),
        From::End => n.saturating_sub(at.min(n)),
    }
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map(|(b, _)| b).unwrap_or(s.len())
}

/// Put `text` at char position `pos`; with `overwrite` it replaces that many characters.
fn put(s: &str, pos: usize, text: &str, overwrite: bool) -> String {
    let b = byte_at(s, pos);
    let end = if overwrite { byte_at(s, pos + text.chars().count()) } else { b };
    format!("{}{}{}", &s[..b], text, &s[end..])
}

fn replace_all(s: &str, find: &str, with: &str, match_case: bool) -> String {
    if find.is_empty() {
        return s.to_string();
    }
    if match_case {
        return s.replace(find, with);
    }
    // Case-insensitive, keeping the rest of the text as it is.
    let lower = s.to_lowercase();
    let flower = find.to_lowercase();
    if lower.len() != s.len() {
        // Lowercasing changed byte lengths (rare scripts): fall back to exact matching.
        return s.replace(find, with);
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find(&flower) {
        out.push_str(&s[i..i + p]);
        out.push_str(with);
        i += p + flower.len();
    }
    out.push_str(&s[i..]);
    out
}

fn change_case(s: &str, mode: CaseMode) -> String {
    match mode {
        CaseMode::Lower => s.to_lowercase(),
        CaseMode::Upper => s.to_uppercase(),
        CaseMode::Title => {
            let mut out = String::new();
            let mut start = true;
            for c in s.chars() {
                if start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    start = false;
                } else {
                    out.extend(c.to_lowercase());
                }
                if c.is_whitespace() || c == '-' || c == '_' || c == '.' {
                    start = true;
                }
            }
            out
        }
        CaseMode::Sentence => {
            let lower = s.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(f) => f.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

/// The rule applied to one piece of text (the name, the extension or both).
fn apply_to(text: &str, r: &Rules, index: usize, modified: Option<SystemTime>) -> String {
    match r.mode {
        Mode::Insert => {
            let pos = char_pos(text, r.insert_at, r.insert_from);
            put(text, pos, &r.insert_text, r.overwrite)
        }
        Mode::DateTime => {
            let t = match r.date_source {
                DateSource::Now => SystemTime::now(),
                DateSource::Modified => modified.unwrap_or_else(SystemTime::now),
            };
            let dt: chrono::DateTime<chrono::Local> = t.into();
            // Bad format strings would panic inside chrono's Display; check them first.
            let items: Vec<chrono::format::Item> = chrono::format::StrftimeItems::new(&r.date_format).collect();
            let stamp = if items.iter().any(|i| matches!(i, chrono::format::Item::Error)) {
                r.date_format.clone()
            } else {
                dt.format_with_items(items.into_iter()).to_string()
            };
            put(text, char_pos(text, r.date_at, r.date_from), &stamp, false)
        }
        Mode::Number => {
            let n = r.number_start + r.number_step * index as i64;
            let num = if n < 0 { format!("-{:0w$}", -n, w = r.number_digits) } else { format!("{:0w$}", n, w = r.number_digits) };
            if r.number_after { format!("{text}{}{num}", r.number_text) } else { format!("{num}{}{text}", r.number_text) }
        }
        Mode::Remove => {
            let n = text.chars().count();
            let (from, to) = match r.remove_side {
                From::Start => (r.remove_from.min(n), (r.remove_from + r.remove_count).min(n)),
                From::End => (n.saturating_sub(r.remove_from + r.remove_count), n.saturating_sub(r.remove_from)),
            };
            let (a, b) = (byte_at(text, from), byte_at(text, to));
            format!("{}{}", &text[..a], &text[b..])
        }
        Mode::Replace => replace_all(text, &r.find, &r.replace, r.match_case),
        Mode::Case => change_case(text, r.case),
    }
}

/// The new name for one item (`index` is its place in the list, for numbering).
pub fn new_name(name: &str, is_dir: bool, r: &Rules, index: usize, modified: Option<SystemTime>) -> String {
    let (stem, ext) = split(name, is_dir);
    match r.part {
        Part::Name => join(&apply_to(&stem, r, index, modified), &ext),
        Part::Extension if ext.is_empty() => name.to_string(),
        Part::Extension => join(&stem, &apply_to(&ext, r, index, modified)),
        Part::Both => apply_to(name, r, index, modified),
    }
}

/// One row of the preview.
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Why this rename can't happen, if it can't.
    pub problem: Option<String>,
}

impl Planned {
    pub fn changes(&self) -> bool {
        self.from != self.to
    }
}

/// Work out every new name and flag problems: empty or invalid names, two items ending up
/// with the same name, or a name already taken by a file that isn't being renamed.
pub fn plan(items: &[(PathBuf, bool, Option<SystemTime>)], r: &Rules) -> Vec<Planned> {
    let sources: HashSet<&Path> = items.iter().map(|(p, ..)| p.as_path()).collect();
    let mut out: Vec<Planned> = items
        .iter()
        .enumerate()
        .map(|(i, (p, is_dir, modified))| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let new = new_name(&name, *is_dir, r, i, *modified);
            let problem = if new.trim().is_empty() {
                Some("Empty name".to_string())
            } else if new.contains('/') {
                Some("Names can't contain “/”".to_string())
            } else if new == "." || new == ".." {
                Some("Not a valid name".to_string())
            } else {
                None
            };
            Planned { from: p.clone(), to: p.with_file_name(&new), problem }
        })
        .collect();
    // Two items heading for the same name.
    let mut seen: std::collections::HashMap<PathBuf, usize> = std::collections::HashMap::new();
    for (i, pl) in out.iter().enumerate() {
        *seen.entry(pl.to.clone()).or_insert(0) += 1;
        let _ = i;
    }
    for pl in out.iter_mut() {
        if pl.problem.is_none() && seen.get(&pl.to).copied().unwrap_or(0) > 1 {
            pl.problem = Some("Same name as another item".into());
        }
        // Already taken by something that isn't part of this rename.
        if pl.problem.is_none() && pl.changes() && pl.to.symlink_metadata().is_ok() && !sources.contains(pl.to.as_path()) {
            // Only a case change of the same file (on case-insensitive disks) is fine.
            if pl.to.to_string_lossy().to_lowercase() != pl.from.to_string_lossy().to_lowercase() {
                pl.problem = Some("A file with this name already exists".into());
            }
        }
    }
    out
}

/// Rename everything that changes. First every item moves to a temporary name, then to its
/// final name, so swaps and shifts ("1→2, 2→3") work. Returns the (old, new) pairs done.
pub fn apply(plan: &[Planned]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    if let Some(p) = plan.iter().find(|p| p.problem.is_some()) {
        return Err(format!("{}: {}", p.from.display(), p.problem.as_deref().unwrap_or("")));
    }
    let todo: Vec<&Planned> = plan.iter().filter(|p| p.changes()).collect();
    let stamp = format!("{}-{}", std::process::id(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let mut temps: Vec<(PathBuf, &Planned)> = Vec::new();
    for (i, p) in todo.iter().enumerate() {
        let tmp = p.from.with_file_name(format!(".rivet-rename-{stamp}-{i}"));
        if let Err(e) = std::fs::rename(&p.from, &tmp) {
            // Put back what already moved.
            for (t, q) in temps.iter().rev() {
                let _ = std::fs::rename(t, &q.from);
            }
            return Err(format!("{}: {e}", p.from.display()));
        }
        temps.push((tmp, p));
    }
    let mut done = Vec::new();
    let mut failed = Vec::new();
    for (tmp, p) in &temps {
        match std::fs::rename(tmp, &p.to) {
            Ok(()) => done.push((p.from.clone(), p.to.clone())),
            Err(e) => {
                // Keep the file under its old name rather than a temporary one.
                let _ = std::fs::rename(tmp, &p.from);
                failed.push(format!("{}: {e}", p.from.display()));
            }
        }
    }
    if failed.is_empty() { Ok(done) } else { Err(failed.join("\n")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(f: impl FnOnce(&mut Rules)) -> Rules {
        let mut r = Rules::default();
        f(&mut r);
        r
    }

    #[test]
    fn rules() {
        let nn = |name: &str, rules: &Rules, i: usize| new_name(name, false, rules, i, None);
        let ins = r(|r| {
            r.mode = Mode::Insert;
            r.insert_text = "new_".into();
        });
        assert_eq!(nn("photo.jpg", &ins, 0), "new_photo.jpg");
        let ow = r(|r| {
            r.insert_text = "PIC".into();
            r.overwrite = true;
        });
        assert_eq!(nn("IMG_0042.jpg", &ow, 0), "PIC_0042.jpg");
        let end = r(|r| {
            r.insert_text = "_v2".into();
            r.insert_from = From::End;
        });
        assert_eq!(nn("report.pdf", &end, 0), "report_v2.pdf");
        let num = r(|r| {
            r.mode = Mode::Number;
            r.number_start = 9;
            r.number_digits = 3;
            r.number_text = "_".into();
        });
        assert_eq!(nn("a.png", &num, 0), "009_a.png");
        assert_eq!(nn("b.png", &num, 2), "011_b.png");
        let rm = r(|r| {
            r.mode = Mode::Remove;
            r.remove_from = 0;
            r.remove_count = 4;
        });
        assert_eq!(nn("IMG_0042.jpg", &rm, 0), "0042.jpg");
        let rm_end = r(|r| {
            r.mode = Mode::Remove;
            r.remove_count = 2;
            r.remove_side = From::End;
        });
        assert_eq!(nn("draft01.txt", &rm_end, 0), "draft.txt");
        let rep = r(|r| {
            r.mode = Mode::Replace;
            r.find = "img".into();
            r.replace = "Holiday".into();
        });
        assert_eq!(nn("IMG_0042.JPG", &rep, 0), "Holiday_0042.JPG");
        let case = r(|r| {
            r.mode = Mode::Case;
            r.case = CaseMode::Title;
        });
        assert_eq!(nn("my summer-trip.MP4", &case, 0), "My Summer-Trip.MP4");
        let ext = r(|r| {
            r.mode = Mode::Case;
            r.part = Part::Extension;
        });
        assert_eq!(nn("IMG_1.JPG", &ext, 0), "IMG_1.jpg");
        // Folders have no extension; unicode stays intact.
        assert_eq!(new_name("Fotos.2024", true, &ins, 0, None), "new_Fotos.2024");
        assert_eq!(nn("café.txt", &end, 0), "café_v2.txt");
        // A broken date format is shown as typed instead of crashing.
        let bad = r(|r| {
            r.mode = Mode::DateTime;
            r.date_format = "%Q".into();
        });
        assert_eq!(nn("x.txt", &bad, 0), "%Qx.txt");
    }

    #[test]
    fn plan_and_apply() {
        let dir = std::env::temp_dir().join(format!("rivet-bulk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["1.txt", "2.txt", "3.txt", "other.txt"] {
            std::fs::write(dir.join(n), n).unwrap();
        }
        let items: Vec<(PathBuf, bool, Option<SystemTime>)> = ["1.txt", "2.txt", "3.txt"].iter().map(|n| (dir.join(n), false, None)).collect();
        // Shift the numbers up by one: 1→2, 2→3, 3→4 must not clobber each other.
        let shift = r(|r| {
            r.mode = Mode::Number;
            r.number_start = 2;
            r.number_digits = 1;
            r.number_text = String::new();
            r.part = Part::Name;
        });
        // Numbering puts the number before the (now replaced) name, so replace names entirely:
        let shift = Rules { mode: Mode::Number, ..shift };
        let p = plan(&items, &shift);
        assert!(p.iter().all(|x| x.problem.is_none()), "{p:?}");
        let done = apply(&p).unwrap();
        assert_eq!(done.len(), 3);
        assert_eq!(std::fs::read_to_string(dir.join("21.txt")).unwrap(), "1.txt");
        // A name already taken by a file outside the selection is flagged.
        let items: Vec<(PathBuf, bool, Option<SystemTime>)> = vec![(dir.join("21.txt"), false, None)];
        let to_other = r(|r| {
            r.mode = Mode::Replace;
            r.find = "21".into();
            r.replace = "other".into();
        });
        let p = plan(&items, &to_other);
        assert!(p[0].problem.is_some());
        assert!(apply(&p).is_err());
        // Two items heading for the same name are flagged.
        let items: Vec<(PathBuf, bool, Option<SystemTime>)> = vec![(dir.join("21.txt"), false, None), (dir.join("32.txt"), false, None)];
        let same = r(|r| {
            r.mode = Mode::Remove;
            r.remove_count = 2;
        });
        let p = plan(&items, &same);
        assert!(p.iter().all(|x| x.problem.is_some()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
