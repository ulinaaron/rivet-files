//! Filters: show a folder with only a certain set of its items (by type, name or date).
//!
//! A filter is global, for one folder, or for a folder and everything under it. Only one
//! applies at a time, and the most specific one wins:
//!   1. a filter on this exact folder,
//!   2. else the nearest parent folder's "& subfolders" filter,
//!   3. else the global filter.
//! Each filter can be switched off without losing its settings.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::model::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeGroup {
    Folders,
    Images,
    Videos,
    Audio,
    Documents,
    Archives,
    Text,
    Other,
}

impl TypeGroup {
    pub const ALL: [TypeGroup; 8] =
        [TypeGroup::Folders, TypeGroup::Images, TypeGroup::Videos, TypeGroup::Audio, TypeGroup::Documents, TypeGroup::Archives, TypeGroup::Text, TypeGroup::Other];

    pub fn label(self) -> &'static str {
        match self {
            TypeGroup::Folders => "Folders",
            TypeGroup::Images => "Pictures",
            TypeGroup::Videos => "Videos",
            TypeGroup::Audio => "Audio",
            TypeGroup::Documents => "Documents",
            TypeGroup::Archives => "Archives",
            TypeGroup::Text => "Code & text",
            TypeGroup::Other => "Other",
        }
    }

    fn id(self) -> &'static str {
        match self {
            TypeGroup::Folders => "folders",
            TypeGroup::Images => "images",
            TypeGroup::Videos => "videos",
            TypeGroup::Audio => "audio",
            TypeGroup::Documents => "documents",
            TypeGroup::Archives => "archives",
            TypeGroup::Text => "text",
            TypeGroup::Other => "other",
        }
    }

    fn from_id(s: &str) -> Option<TypeGroup> {
        TypeGroup::ALL.into_iter().find(|t| t.id() == s)
    }

    /// Which group an item belongs to.
    pub fn of(e: &Entry) -> TypeGroup {
        if e.is_dir {
            return TypeGroup::Folders;
        }
        let m = e.mime.as_str();
        let doc = m == "application/pdf"
            || m.contains("opendocument")
            || m.contains("officedocument")
            || m.contains("msword")
            || m.contains("ms-excel")
            || m.contains("ms-powerpoint")
            || m.contains("rtf")
            || m.contains("epub");
        if m.starts_with("image/") {
            TypeGroup::Images
        } else if m.starts_with("video/") {
            TypeGroup::Videos
        } else if m.starts_with("audio/") {
            TypeGroup::Audio
        } else if doc {
            TypeGroup::Documents
        } else if crate::archive::is_archive_name(&e.name) {
            TypeGroup::Archives
        } else if crate::preview::kind_of(m) == crate::preview::Kind::Text {
            TypeGroup::Text
        } else {
            TypeGroup::Other
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameMode {
    /// The name contains the text.
    Contains,
    /// The name doesn't contain the text.
    Excludes,
    /// The name matches one of these patterns (`*.jpg, IMG_*`).
    Includes,
}

impl NameMode {
    fn id(self) -> &'static str {
        match self {
            NameMode::Contains => "contains",
            NameMode::Excludes => "excludes",
            NameMode::Includes => "includes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateRange {
    Any,
    Today,
    Week,
    Month,
    Year,
    OlderThanYear,
}

impl DateRange {
    pub const ALL: [DateRange; 6] = [DateRange::Any, DateRange::Today, DateRange::Week, DateRange::Month, DateRange::Year, DateRange::OlderThanYear];

    pub fn label(self) -> &'static str {
        match self {
            DateRange::Any => "Any time",
            DateRange::Today => "Today",
            DateRange::Week => "Past week",
            DateRange::Month => "Past month",
            DateRange::Year => "Past year",
            DateRange::OlderThanYear => "Older than a year",
        }
    }

    fn id(self) -> &'static str {
        match self {
            DateRange::Any => "any",
            DateRange::Today => "today",
            DateRange::Week => "week",
            DateRange::Month => "month",
            DateRange::Year => "year",
            DateRange::OlderThanYear => "older",
        }
    }

    fn matches(self, modified: Option<SystemTime>, now: SystemTime) -> bool {
        let day = Duration::from_secs(86_400);
        let age = modified.and_then(|m| now.duration_since(m).ok()).unwrap_or(Duration::ZERO);
        match self {
            DateRange::Any => true,
            DateRange::Today => {
                let (Some(m), true) = (modified, true) else { return false };
                let a: chrono::DateTime<chrono::Local> = m.into();
                let b: chrono::DateTime<chrono::Local> = now.into();
                a.date_naive() == b.date_naive()
            }
            DateRange::Week => modified.is_some() && age <= day * 7,
            DateRange::Month => modified.is_some() && age <= day * 31,
            DateRange::Year => modified.is_some() && age <= day * 366,
            DateRange::OlderThanYear => modified.is_some() && age > day * 366,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub enabled: bool,
    /// Empty: every type.
    pub types: Vec<TypeGroup>,
    pub name_mode: NameMode,
    pub name: String,
    pub date: DateRange,
}

impl Default for Filter {
    fn default() -> Self {
        Filter { enabled: true, types: Vec::new(), name_mode: NameMode::Contains, name: String::new(), date: DateRange::Any }
    }
}

impl Filter {
    /// Does this filter actually narrow anything down?
    pub fn is_active(&self) -> bool {
        self.enabled && (!self.types.is_empty() || !self.name.trim().is_empty() || self.date != DateRange::Any)
    }

    pub fn matches(&self, e: &Entry, now: SystemTime) -> bool {
        if !self.types.is_empty() && !self.types.contains(&TypeGroup::of(e)) {
            return false;
        }
        let text = self.name.trim();
        if !text.is_empty() {
            let name = e.name.to_lowercase();
            let ok = match self.name_mode {
                NameMode::Contains => name.contains(&text.to_lowercase()),
                NameMode::Excludes => !name.contains(&text.to_lowercase()),
                NameMode::Includes => text
                    .split(',')
                    .map(|p| p.trim().to_lowercase())
                    .filter(|p| !p.is_empty())
                    .any(|p| if p.contains('*') || p.contains('?') { crate::mime::glob_match(&p, &name) } else { name.contains(&p) }),
            };
            if !ok {
                return false;
            }
        }
        self.date.matches(e.modified, now)
    }

    /// A short description: "Pictures, Videos · name contains “IMG” · past week".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.types.is_empty() {
            parts.push(self.types.iter().map(|t| t.label()).collect::<Vec<_>>().join(", "));
        }
        let text = self.name.trim();
        if !text.is_empty() {
            parts.push(match self.name_mode {
                NameMode::Contains => format!("name contains “{text}”"),
                NameMode::Excludes => format!("name without “{text}”"),
                NameMode::Includes => format!("names like {text}"),
            });
        }
        if self.date != DateRange::Any {
            parts.push(self.date.label().to_lowercase());
        }
        if parts.is_empty() { "everything".into() } else { parts.join(" · ") }
    }
}

/// Where a filter applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Folder(PathBuf),
    /// A folder and everything under it.
    Tree(PathBuf),
}

impl Scope {
    pub fn label(&self) -> String {
        match self {
            Scope::Global => "Global".into(),
            Scope::Folder(_) => "This folder".into(),
            Scope::Tree(p) => {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());
                format!("“{name}” & subfolders")
            }
        }
    }
}

/// Set for check / screenshot runs: filters are then never written to disk.
pub static NO_SAVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilterSet {
    pub global: Option<Filter>,
    /// (folder, includes subfolders, filter)
    pub folders: Vec<(PathBuf, bool, Filter)>,
}

impl FilterSet {
    pub fn get(&self, scope: &Scope) -> Option<&Filter> {
        match scope {
            Scope::Global => self.global.as_ref(),
            Scope::Folder(p) => self.folders.iter().find(|(d, tree, _)| d == p && !*tree).map(|(.., f)| f),
            Scope::Tree(p) => self.folders.iter().find(|(d, tree, _)| d == p && *tree).map(|(.., f)| f),
        }
    }

    /// The filter for this scope, created (empty) if needed.
    pub fn get_mut(&mut self, scope: &Scope) -> &mut Filter {
        match scope {
            Scope::Global => self.global.get_or_insert_with(Filter::default),
            Scope::Folder(p) | Scope::Tree(p) => {
                let tree = matches!(scope, Scope::Tree(_));
                if let Some(i) = self.folders.iter().position(|(d, t, _)| d == p && *t == tree) {
                    &mut self.folders[i].2
                } else {
                    self.folders.push((p.clone(), tree, Filter::default()));
                    &mut self.folders.last_mut().unwrap().2
                }
            }
        }
    }

    pub fn remove(&mut self, scope: &Scope) {
        match scope {
            Scope::Global => self.global = None,
            Scope::Folder(p) => self.folders.retain(|(d, t, _)| !(d == p && !*t)),
            Scope::Tree(p) => self.folders.retain(|(d, t, _)| !(d == p && *t)),
        }
    }

    /// The filter that applies in `dir` (most specific first), if any is switched on and
    /// narrows something down.
    pub fn active_for(&self, dir: &Path) -> Option<(Scope, &Filter)> {
        if let Some(f) = self.get(&Scope::Folder(dir.to_path_buf())).filter(|f| f.is_active()) {
            return Some((Scope::Folder(dir.to_path_buf()), f));
        }
        // The nearest parent with an "& subfolders" filter (the folder itself counts too).
        let mut best: Option<(&PathBuf, &Filter)> = None;
        for (d, tree, f) in &self.folders {
            if *tree && f.is_active() && dir.starts_with(d) && best.is_none_or(|(b, _)| d.components().count() > b.components().count()) {
                best = Some((d, f));
            }
        }
        if let Some((d, f)) = best {
            return Some((Scope::Tree(d.clone()), f));
        }
        self.global.as_ref().filter(|f| f.is_active()).map(|f| (Scope::Global, f))
    }

    /// Which less specific filters the active one is hiding (for "overrides Global").
    pub fn overridden(&self, dir: &Path) -> Vec<Scope> {
        let Some((active, _)) = self.active_for(dir) else { return Vec::new() };
        let mut out = Vec::new();
        if matches!(active, Scope::Folder(_)) {
            for (d, tree, f) in &self.folders {
                if *tree && f.is_active() && dir.starts_with(d) {
                    out.push(Scope::Tree(d.clone()));
                }
            }
        }
        if !matches!(active, Scope::Global) && self.global.as_ref().is_some_and(|f| f.is_active()) {
            out.push(Scope::Global);
        }
        out
    }

    // ---- saving: ~/.config/rivet/filters

    fn path() -> PathBuf {
        crate::places::config_home().join("rivet/filters")
    }

    pub fn load() -> FilterSet {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        // Check / screenshot runs never touch the real filters file.
        if NO_SAVE.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let p = Self::path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, self.to_text());
    }

    fn write_filter(out: &mut String, f: &Filter) {
        let types: Vec<&str> = f.types.iter().map(|t| t.id()).collect();
        out.push_str(&format!("enabled={}\ntypes={}\nname_mode={}\nname={}\ndate={}\n\n", f.enabled, types.join(","), f.name_mode.id(), f.name, f.date.id()));
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if let Some(g) = &self.global {
            out.push_str("[global]\n");
            Self::write_filter(&mut out, g);
        }
        for (d, tree, f) in &self.folders {
            out.push_str(&format!("[{} {}]\n", if *tree { "tree" } else { "folder" }, d.display()));
            Self::write_filter(&mut out, f);
        }
        out
    }

    pub fn parse(text: &str) -> FilterSet {
        let mut set = FilterSet::default();
        let mut cur: Option<(Scope, Filter)> = None;
        let flush = |cur: &mut Option<(Scope, Filter)>, set: &mut FilterSet| {
            if let Some((scope, f)) = cur.take() {
                *set.get_mut(&scope) = f;
            }
        };
        for line in text.lines() {
            let line = line.trim_end();
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                flush(&mut cur, &mut set);
                let scope = if head == "global" {
                    Some(Scope::Global)
                } else if let Some(p) = head.strip_prefix("folder ") {
                    Some(Scope::Folder(PathBuf::from(p)))
                } else {
                    head.strip_prefix("tree ").map(|p| Scope::Tree(PathBuf::from(p)))
                };
                cur = scope.map(|s| (s, Filter::default()));
                continue;
            }
            let (Some((_, f)), Some((k, v))) = (cur.as_mut(), line.split_once('=')) else { continue };
            match k {
                "enabled" => f.enabled = v == "true",
                "types" => f.types = v.split(',').filter_map(TypeGroup::from_id).collect(),
                "name_mode" => {
                    f.name_mode = match v {
                        "excludes" => NameMode::Excludes,
                        "includes" => NameMode::Includes,
                        _ => NameMode::Contains,
                    }
                }
                "name" => f.name = v.to_string(),
                "date" => f.date = DateRange::ALL.into_iter().find(|d| d.id() == v).unwrap_or(DateRange::Any),
                _ => {}
            }
        }
        flush(&mut cur, &mut set);
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, mime: &str, is_dir: bool, days_old: u64) -> Entry {
        Entry {
            path: PathBuf::from("/x").join(name),
            name: name.into(),
            is_dir,
            is_link: false,
            broken_link: false,
            size: 1,
            modified: Some(SystemTime::now() - Duration::from_secs(days_old * 86_400 + 60)),
            mime: mime.into(),
            hidden: false,
            mode: 0o644,
            uid: 0,
            child_count: None,
            trash_origin: None,
        }
    }

    #[test]
    fn matching() {
        let now = SystemTime::now();
        let pic = entry("IMG_0042.jpg", "image/jpeg", false, 2);
        let doc = entry("report.pdf", "application/pdf", false, 400);
        let dir = entry("Holiday", "inode/directory", true, 1);
        let f = Filter { types: vec![TypeGroup::Images], ..Default::default() };
        assert!(f.matches(&pic, now) && !f.matches(&doc, now) && !f.matches(&dir, now));
        let f = Filter { name: "img".into(), ..Default::default() };
        assert!(f.matches(&pic, now) && !f.matches(&doc, now));
        let f = Filter { name: "img".into(), name_mode: NameMode::Excludes, ..Default::default() };
        assert!(!f.matches(&pic, now) && f.matches(&doc, now));
        let f = Filter { name: "*.pdf, holi*".into(), name_mode: NameMode::Includes, ..Default::default() };
        assert!(!f.matches(&pic, now) && f.matches(&doc, now) && f.matches(&dir, now));
        let f = Filter { date: DateRange::Week, ..Default::default() };
        assert!(f.matches(&pic, now) && !f.matches(&doc, now));
        let f = Filter { date: DateRange::OlderThanYear, ..Default::default() };
        assert!(f.matches(&doc, now) && !f.matches(&pic, now));
        assert_eq!(Filter { types: vec![TypeGroup::Images], name: "IMG".into(), ..Default::default() }.summary(), "Pictures · name contains “IMG”");
    }

    #[test]
    fn precedence_and_saving() {
        let images = Filter { types: vec![TypeGroup::Images], ..Default::default() };
        let docs = Filter { types: vec![TypeGroup::Documents], ..Default::default() };
        let audio = Filter { types: vec![TypeGroup::Audio], ..Default::default() };
        let mut set = FilterSet::default();
        set.global = Some(images.clone());
        *set.get_mut(&Scope::Tree("/home/me".into())) = docs.clone();
        *set.get_mut(&Scope::Folder("/home/me/Music".into())) = audio.clone();
        // Folder beats the parent's subfolder filter, which beats global.
        assert_eq!(set.active_for(Path::new("/home/me/Music")).unwrap().0, Scope::Folder("/home/me/Music".into()));
        assert_eq!(set.active_for(Path::new("/home/me/Work")).unwrap().0, Scope::Tree("/home/me".into()));
        assert_eq!(set.active_for(Path::new("/tmp")).unwrap().0, Scope::Global);
        assert_eq!(set.overridden(Path::new("/home/me/Music")).len(), 2);
        // Switched off: the next one down applies.
        set.get_mut(&Scope::Folder("/home/me/Music".into())).enabled = false;
        assert_eq!(set.active_for(Path::new("/home/me/Music")).unwrap().0, Scope::Tree("/home/me".into()));
        // Round trip through the file format.
        let back = FilterSet::parse(&set.to_text());
        assert_eq!(back, set);
    }
}
