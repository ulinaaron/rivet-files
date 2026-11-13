//! Keep check / screenshot windows out of the way on niri.
//!
//! In `--screenshot` and `--measure` runs, Rivet moves its own window to a background
//! workspace as soon as niri lists it, without moving your focus. Target, in order:
//! the workspace named in RIVET_CHECK_WORKSPACE (or one named "rivet-checks"), else the
//! empty workspace at the end of the window's monitor. RIVET_CHECK_WORKSPACE=off disables it.
//!
//! For no flash at all, add a niri window rule for the `rivet-files-check` app id
//! (see the README).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// App id used by check / screenshot windows, so window rules can target only them.
pub const CHECK_APP_ID: &str = "rivet-files-check";

static MOVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True once this window sits on a workspace you aren't looking at.
pub fn moved_away() -> bool {
    MOVED.load(std::sync::atomic::Ordering::Relaxed)
}

fn niri_json(args: &[&str]) -> Option<serde_like::Value> {
    let out = Command::new("niri").arg("msg").arg("--json").args(args).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_like::parse(&String::from_utf8_lossy(&out.stdout))
}

/// Start the background move (returns at once). Does nothing outside niri.
pub fn send_check_window_away() {
    if std::env::var_os("NIRI_SOCKET").is_none() {
        return;
    }
    let wanted = std::env::var("RIVET_CHECK_WORKSPACE").unwrap_or_default();
    if wanted == "off" {
        return;
    }
    let pid = std::process::id() as i64;
    std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(8) {
            std::thread::sleep(Duration::from_millis(15));
            let Some(windows) = niri_json(&["windows"]) else { return };
            let Some(win) = windows.items().into_iter().find(|w| w.get("pid").and_then(|p| p.as_i64()) == Some(pid)) else { continue };
            let Some(win_id) = win.get("id").and_then(|v| v.as_i64()) else { return };
            let win_ws = win.get("workspace_id").and_then(|v| v.as_i64());
            let Some(spaces) = niri_json(&["workspaces"]) else { return };
            // Already somewhere you aren't looking (e.g. a window rule put it there)?
            let on_active = spaces.items().iter().any(|w| w.get("id").and_then(|v| v.as_i64()) == win_ws && matches!(w.get("is_active"), Some(serde_like::Value::Bool(true))));
            if !on_active {
                MOVED.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            let spaces = spaces.items();
            let name_of = |w: &serde_like::Value| w.get("name").and_then(|n| n.as_str().map(|s| s.to_string()));
            let target_name = if wanted.is_empty() { "rivet-checks".to_string() } else { wanted.clone() };
            let target = if spaces.iter().any(|w| name_of(w).as_deref() == Some(target_name.as_str())) {
                target_name
            } else {
                // The empty workspace niri keeps at the end of the window's monitor.
                let output = spaces.iter().find(|w| w.get("id").and_then(|v| v.as_i64()) == win_ws).and_then(|w| w.get("output").and_then(|o| o.as_str().map(|s| s.to_string())));
                let last = spaces
                    .iter()
                    .filter(|w| output.is_none() || w.get("output").and_then(|o| o.as_str().map(|s| s.to_string())) == output)
                    .filter_map(|w| w.get("idx").and_then(|v| v.as_i64()))
                    .max();
                match last {
                    Some(i) => i.to_string(),
                    None => return,
                }
            };
            let ok = Command::new("niri")
                .args(["msg", "action", "move-window-to-workspace", "--window-id", &win_id.to_string(), "--focus", "false", &target])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            MOVED.store(ok, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    });
}

/// A tiny JSON reader, just enough for niri's replies (no extra dependency).
mod serde_like {
    #[derive(Debug, Clone)]
    pub enum Value {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Value>),
        Obj(Vec<(String, Value)>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn items(&self) -> Vec<Value> {
            match self {
                Value::Arr(v) => v.clone(),
                _ => Vec::new(),
            }
        }
        pub fn as_i64(&self) -> Option<i64> {
            match self {
                Value::Num(n) => Some(*n as i64),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }
    }

    pub fn parse(s: &str) -> Option<Value> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i)?;
        Some(v)
    }

    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    }

    fn value(b: &[u8], i: &mut usize) -> Option<Value> {
        ws(b, i);
        match *b.get(*i)? {
            b'{' => {
                *i += 1;
                let mut kv = Vec::new();
                loop {
                    ws(b, i);
                    if *b.get(*i)? == b'}' {
                        *i += 1;
                        return Some(Value::Obj(kv));
                    }
                    let Value::Str(k) = string(b, i)? else { return None };
                    ws(b, i);
                    if *b.get(*i)? != b':' {
                        return None;
                    }
                    *i += 1;
                    kv.push((k, value(b, i)?));
                    ws(b, i);
                    if *b.get(*i)? == b',' {
                        *i += 1;
                    }
                }
            }
            b'[' => {
                *i += 1;
                let mut v = Vec::new();
                loop {
                    ws(b, i);
                    if *b.get(*i)? == b']' {
                        *i += 1;
                        return Some(Value::Arr(v));
                    }
                    v.push(value(b, i)?);
                    ws(b, i);
                    if *b.get(*i)? == b',' {
                        *i += 1;
                    }
                }
            }
            b'"' => string(b, i),
            b't' => {
                *i += 4;
                Some(Value::Bool(true))
            }
            b'f' => {
                *i += 5;
                Some(Value::Bool(false))
            }
            b'n' => {
                *i += 4;
                Some(Value::Null)
            }
            _ => {
                let start = *i;
                while *i < b.len() && (b[*i].is_ascii_digit() || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E')) {
                    *i += 1;
                }
                std::str::from_utf8(&b[start..*i]).ok()?.parse().ok().map(Value::Num)
            }
        }
    }

    fn string(b: &[u8], i: &mut usize) -> Option<Value> {
        if *b.get(*i)? != b'"' {
            return None;
        }
        *i += 1;
        let mut out = Vec::new();
        while *i < b.len() {
            match b[*i] {
                b'"' => {
                    *i += 1;
                    return Some(Value::Str(String::from_utf8_lossy(&out).into_owned()));
                }
                b'\\' => {
                    *i += 1;
                    match *b.get(*i)? {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(b.get(*i + 1..*i + 5)?).ok()?;
                            let c = char::from_u32(u32::from_str_radix(hex, 16).ok()?).unwrap_or('?');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                            *i += 4;
                        }
                        c => out.push(c),
                    }
                    *i += 1;
                }
                c => {
                    out.push(c);
                    *i += 1;
                }
            }
        }
        None
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn reads_niri_like_json() {
            let v = super::parse(r#"[{"id":12,"pid":345,"title":"a \"b\"","workspace_id":3,"is_floating":false,"layout":{"x":1.5}},{"id":13,"pid":null}]"#).unwrap();
            let items = v.items();
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].get("pid").unwrap().as_i64(), Some(345));
            assert_eq!(items[0].get("title").unwrap().as_str(), Some("a \"b\""));
            assert_eq!(items[0].get("workspace_id").unwrap().as_i64(), Some(3));
        }
    }
}
