//! File type detection from the shared-mime-info database (plain data files in
//! /usr/share/mime), plus icon names and readable descriptions for each type.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct MimeDb {
    ext: HashMap<String, (u32, String)>,  // "png" -> mime  (from *.png)
    literal: HashMap<String, String>,     // "makefile" -> mime
    patterns: Vec<(u32, String, String)>, // other globs (weight, pattern, mime)
    /// For each pattern: the literal text before its first and after its last wildcard.
    pattern_fix: Vec<(String, String)>,
    generic_icons: HashMap<String, String>,
    icons: HashMap<String, String>,
    parents: HashMap<String, Vec<String>>,
    comments: std::sync::Mutex<HashMap<String, String>>,
    roots: Vec<PathBuf>,
}

fn mime_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        v.push(home.join(".local/share/mime"));
    }
    let dd = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in dd.split(':') {
        let p = PathBuf::from(d).join("mime");
        if !v.contains(&p) {
            v.push(p);
        }
    }
    for d in crate::places::host_data_dirs() {
        let p = d.join("mime");
        if p.is_dir() && !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

impl MimeDb {
    pub fn load() -> Self {
        let mut db = MimeDb {
            ext: HashMap::new(),
            literal: HashMap::new(),
            patterns: Vec::new(),
            pattern_fix: Vec::new(),
            generic_icons: HashMap::new(),
            icons: HashMap::new(),
            parents: HashMap::new(),
            comments: Default::default(),
            roots: mime_roots(),
        };
        let roots = db.roots.clone();
        for root in roots.iter().rev() {
            if let Ok(t) = std::fs::read_to_string(root.join("globs2")) {
                for line in t.lines() {
                    if line.starts_with('#') {
                        continue;
                    }
                    let mut parts = line.splitn(4, ':');
                    let (Some(w), Some(mime), Some(glob)) = (parts.next(), parts.next(), parts.next()) else { continue };
                    let w: u32 = w.parse().unwrap_or(50);
                    let glob_l = glob.to_lowercase();
                    if let Some(e) = glob_l.strip_prefix("*.") {
                        if !e.contains(['*', '?', '[']) {
                            let entry = db.ext.entry(e.to_string()).or_insert((w, mime.to_string()));
                            if w > entry.0 {
                                *entry = (w, mime.to_string());
                            }
                            continue;
                        }
                    }
                    if !glob_l.contains(['*', '?', '[']) {
                        db.literal.insert(glob_l, mime.to_string());
                    } else {
                        db.patterns.push((w, glob_l, mime.to_string()));
                    }
                }
            }
            for (file, map) in [("generic-icons", 0), ("icons", 1)] {
                if let Ok(t) = std::fs::read_to_string(root.join(file)) {
                    for line in t.lines() {
                        if let Some((m, i)) = line.split_once(':') {
                            if map == 0 {
                                db.generic_icons.insert(m.to_string(), i.to_string());
                            } else {
                                db.icons.insert(m.to_string(), i.to_string());
                            }
                        }
                    }
                }
            }
            if let Ok(t) = std::fs::read_to_string(root.join("subclasses")) {
                for line in t.lines() {
                    if let Some((m, p)) = line.split_once(' ') {
                        db.parents.entry(m.to_string()).or_default().push(p.to_string());
                    }
                }
            }
        }
        db.pattern_fix = db
            .patterns
            .iter()
            .map(|(_, pat, _)| {
                let is_wild = |c: char| matches!(c, '*' | '?' | '[');
                let pre: String = pat.chars().take_while(|c| !is_wild(*c)).collect();
                let suf: String = {
                    let mut v: Vec<char> = pat.chars().rev().take_while(|c| !is_wild(*c) && *c != ']').collect();
                    v.reverse();
                    v.into_iter().collect()
                };
                (pre, suf)
            })
            .collect();
        db
    }

    /// Guess a type from the file name; falls back to a quick look at the content.
    /// Type from the file name alone (no disk access); None if the name says nothing.
    pub fn guess_by_name(&self, path: &Path) -> Option<String> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        if let Some(m) = self.literal.get(&name) {
            return Some(m.clone());
        }
        let mut idx = 0;
        while let Some(pos) = name[idx..].find('.') {
            if let Some((_, m)) = self.ext.get(&name[idx + pos + 1..]) {
                return Some(m.clone());
            }
            idx += pos + 1;
        }
        let mut best: Option<&(u32, String, String)> = None;
        for (p, (pre, suf)) in self.patterns.iter().zip(&self.pattern_fix) {
            // Cheap check on the fixed start/end before the full wildcard match.
            if !name.starts_with(pre.as_str()) || !name.ends_with(suf.as_str()) || name.len() < pre.len() + suf.len() {
                continue;
            }
            if glob_match(&p.1, &name) && best.is_none_or(|b| p.0 > b.0) {
                best = Some(p);
            }
        }
        best.map(|b| b.2.clone())
    }

    pub fn guess(&self, path: &Path, is_dir: bool, executable: bool) -> String {
        if is_dir {
            return "inode/directory".into();
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        if let Some(m) = self.literal.get(&name) {
            return m.clone();
        }
        // Longest extension first: archive.tar.gz -> tar.gz before gz
        let mut idx = 0;
        while let Some(pos) = name[idx..].find('.') {
            let e = &name[idx + pos + 1..];
            if let Some((_, m)) = self.ext.get(e) {
                return m.clone();
            }
            idx += pos + 1;
        }
        let mut best: Option<&(u32, String, String)> = None;
        for (p, (pre, suf)) in self.patterns.iter().zip(&self.pattern_fix) {
            // Cheap check on the fixed start/end before the full wildcard match.
            if !name.starts_with(pre.as_str()) || !name.ends_with(suf.as_str()) || name.len() < pre.len() + suf.len() {
                continue;
            }
            if glob_match(&p.1, &name) && best.is_none_or(|b| p.0 > b.0) {
                best = Some(p);
            }
        }
        if let Some(b) = best {
            return b.2.clone();
        }
        // Peek at the first bytes.
        if let Ok(mut f) = std::fs::File::open(path) {
            use std::io::Read;
            let mut buf = [0u8; 512];
            let n = f.read(&mut buf).unwrap_or(0);
            let b = &buf[..n];
            if n == 0 {
                return "application/x-zerosize".into();
            }
            if b.starts_with(b"\x7fELF") {
                return "application/x-executable".into();
            }
            if b.starts_with(b"#!") {
                return "application/x-shellscript".into();
            }
            if b.starts_with(b"\x89PNG") {
                return "image/png".into();
            }
            if b.starts_with(b"%PDF") {
                return "application/pdf".into();
            }
            if std::str::from_utf8(b).is_ok() || !b.contains(&0) {
                return "text/plain".into();
            }
        }
        if executable {
            return "application/x-executable".into();
        }
        "application/octet-stream".into()
    }

    /// Icon names to try for a type, most specific first.
    pub fn icon_names(&self, mime: &str) -> Vec<String> {
        let mut v = Vec::new();
        if mime == "inode/directory" {
            return vec!["folder".into(), "inode-directory".into()];
        }
        if let Some(i) = self.icons.get(mime) {
            v.push(i.clone());
        }
        v.push(mime.replace('/', "-"));
        if let Some(i) = self.generic_icons.get(mime) {
            v.push(i.clone());
        }
        if let Some(ps) = self.parents.get(mime) {
            for p in ps {
                v.push(p.replace('/', "-"));
                if let Some(i) = self.generic_icons.get(p) {
                    v.push(i.clone());
                }
            }
        }
        let media = mime.split('/').next().unwrap_or("");
        v.push(format!("{media}-x-generic"));
        match media {
            "text" => v.push("text-x-generic".into()),
            "audio" | "video" | "image" | "font" => {}
            _ => {}
        }
        v.push("text-x-generic".into());
        v.push("application-x-generic".into());
        v.push("unknown".into());
        v
    }

    /// Human-readable description, e.g. "PNG image".
    pub fn describe(&self, mime: &str) -> String {
        if let Some(c) = self.comments.lock().unwrap().get(mime) {
            return c.clone();
        }
        let mut desc = None;
        for r in &self.roots {
            let p = r.join(format!("{mime}.xml"));
            if let Ok(t) = std::fs::read_to_string(&p) {
                if let Some(start) = t.find("<comment>") {
                    if let Some(end) = t[start..].find("</comment>") {
                        desc = Some(decode_xml(&t[start + 9..start + end]));
                        break;
                    }
                }
            }
        }
        let d = desc.unwrap_or_else(|| match mime {
            "inode/directory" => "Folder".into(),
            _ => mime.to_string(),
        });
        self.comments.lock().unwrap().insert(mime.to_string(), d.clone());
        d
    }

    pub fn is_subclass_of(&self, mime: &str, parent: &str) -> bool {
        if mime == parent {
            return true;
        }
        let mut stack = vec![mime.to_string()];
        let mut guard = 0;
        while let Some(m) = stack.pop() {
            guard += 1;
            if guard > 50 {
                break;
            }
            if let Some(ps) = self.parents.get(&m) {
                for p in ps {
                    if p == parent {
                        return true;
                    }
                    stack.push(p.clone());
                }
            }
        }
        false
    }
}

fn decode_xml(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

/// Simple glob matcher supporting `*`, `?` and `[...]`.
pub fn glob_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = s.chars().collect();
    fn rec(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            '*' => (0..=t.len()).any(|i| rec(&p[1..], &t[i..])),
            '?' => !t.is_empty() && rec(&p[1..], &t[1..]),
            '[' => {
                if t.is_empty() {
                    return false;
                }
                if let Some(end) = p.iter().position(|&c| c == ']') {
                    let set = &p[1..end];
                    let (neg, set) = if set.first() == Some(&'!') { (true, &set[1..]) } else { (false, set) };
                    let mut hit = false;
                    let mut i = 0;
                    while i < set.len() {
                        if i + 2 < set.len() && set[i + 1] == '-' {
                            if set[i] <= t[0] && t[0] <= set[i + 2] {
                                hit = true;
                            }
                            i += 3;
                        } else {
                            if set[i] == t[0] {
                                hit = true;
                            }
                            i += 1;
                        }
                    }
                    hit != neg && rec(&p[end + 1..], &t[1..])
                } else {
                    t[0] == '[' && rec(&p[1..], &t[1..])
                }
            }
            c => !t.is_empty() && t[0] == c && rec(&p[1..], &t[1..]),
        }
    }
    rec(&p, &t)
}
