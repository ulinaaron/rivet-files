//! Network browsing through the desktop's gvfs service, driven by its `gio`
//! command-line tool (no GLib linked): discover servers (`network:///`), list a
//! server's shares, connect (answering gio's login prompts) and find the local
//! folder gvfs exposes for a connected share (/run/user/UID/gvfs/…).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct NetPlace {
    pub name: String,
    pub uri: String,
}

impl NetPlace {
    pub fn scheme(&self) -> &str {
        self.uri.split("://").next().unwrap_or("")
    }
    /// A server whose top level lists shares rather than files (SMB).
    pub fn lists_shares(&self) -> bool {
        self.scheme() == "smb" && share_of(&self.uri).is_none()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Login {
    pub user: String,
    pub domain: String,
    pub password: String,
    pub anonymous: bool,
}

#[derive(Debug)]
pub enum MountError {
    /// The server asked for a user name / password we don't have yet.
    NeedsLogin,
    Failed(String),
}

fn gvfs_dir() -> PathBuf {
    PathBuf::from(format!("/run/user/{}/gvfs", unsafe { libc::getuid() }))
}

/// "smb://host:445/share/sub" -> "host"
pub fn host_of(uri: &str) -> String {
    let rest = uri.split("://").nth(1).unwrap_or("");
    let auth = rest.split('/').next().unwrap_or("");
    let host = auth.rsplit('@').next().unwrap_or("");
    host.split(':').next().unwrap_or("").to_lowercase()
}

/// "smb://host/share/…" -> Some("share")
pub fn share_of(uri: &str) -> Option<String> {
    let rest = uri.split("://").nth(1)?;
    let share = rest.split('/').nth(1)?;
    if share.is_empty() { None } else { Some(share.to_string()) }
}

/// Parse `gio list -a standard::display-name,standard::target-uri …` output.
pub fn parse_gio_list(text: &str, base: Option<&str>) -> Vec<NetPlace> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cols = line.splitn(4, '\t');
        let name = cols.next().unwrap_or("").to_string();
        let attrs = cols.nth(2).unwrap_or("");
        let get = |key: &str| {
            let k = format!("standard::{key}=");
            attrs.find(&k).map(|i| {
                let v = &attrs[i + k.len()..];
                // Values run until the next " standard::" attribute.
                v.split(" standard::").next().unwrap_or("").trim().to_string()
            })
        };
        let display = get("display-name").filter(|s| !s.is_empty()).unwrap_or_else(|| name.clone());
        let uri = match get("target-uri").filter(|s| !s.is_empty()) {
            Some(u) => u,
            None => match base {
                Some(b) => format!("{}/{}/", b.trim_end_matches('/'), name),
                None => continue,
            },
        };
        if !display.ends_with('$') {
            out.push(NetPlace { name: display, uri });
        }
    }
    out
}

fn gio_list(uri: &str, timeout: Duration) -> Result<String, String> {
    let mut child = Command::new("gio")
        .args(["list", "-a", "standard::display-name,standard::target-uri", uri])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("gio is not available: {e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let mut out = String::new();
            let mut err = String::new();
            let _ = child.stdout.take().unwrap().read_to_string(&mut out);
            let _ = child.stderr.take().unwrap().read_to_string(&mut err);
            return if status.success() { Ok(out) } else { Err(err.trim().trim_start_matches("gio: ").to_string()) };
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            return Err("The network did not answer in time.".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Servers announced on the local network.
pub fn discover() -> Vec<NetPlace> {
    let all = gio_list("network:///", Duration::from_secs(8)).map(|t| parse_gio_list(&t, None)).unwrap_or_default();
    dedupe(all)
}

/// A server announced twice (e.g. "WDMyCloud" and "WDMYCLOUD") shows once,
/// keeping the nicer-looking (not all capitals) name.
fn dedupe(all: Vec<NetPlace>) -> Vec<NetPlace> {
    let mut out: Vec<NetPlace> = Vec::new();
    for p in all {
        let key = |x: &NetPlace| (x.scheme().to_string(), x.name.to_lowercase());
        if let Some(i) = out.iter().position(|o| key(o) == key(&p)) {
            let shouty = |n: &str| n.chars().any(|c| c.is_alphabetic()) && n == n.to_uppercase();
            if shouty(&out[i].name) && !shouty(&p.name) {
                out[i] = p;
            }
        } else {
            out.push(p);
        }
    }
    out
}

/// The shares on an SMB server (the server must be reachable; may need a login).
pub fn list_shares(server: &NetPlace) -> Result<Vec<NetPlace>, String> {
    let base = server.uri.trim_end_matches('/').to_string();
    gio_list(&server.uri, Duration::from_secs(10)).map(|t| {
        parse_gio_list(&t, Some(&base))
            .into_iter()
            .map(|mut p| {
                // Shares come back with their own target; keep them under this server.
                if share_of(&p.uri).is_none() {
                    p.uri = format!("{base}/{}/", p.name);
                }
                p
            })
            .collect()
    })
}

/// Connected shares gvfs exposes as folders: (readable name, folder).
pub fn connected() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(gvfs_dir())
        .map(|rd| rd.flatten().map(|e| (pretty_mount_name(&e.file_name().to_string_lossy()), e.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// "smb-share:server=nas.local,share=public" -> "public on nas.local"
pub fn pretty_mount_name(raw: &str) -> String {
    let (kind, rest) = raw.split_once(':').unwrap_or((raw, ""));
    let field = |k: &str| rest.split(',').find_map(|kv| kv.strip_prefix(&format!("{k}=")).map(|s| s.to_string()));
    let server = field("server").or_else(|| field("host")).unwrap_or_default();
    match (kind, field("share")) {
        (_, Some(share)) => format!("{share} on {server}"),
        ("sftp", _) => format!("{server} (SFTP)"),
        ("ftp", _) => format!("{server} (FTP)"),
        ("dav" | "davs", _) => format!("{server} (WebDAV)"),
        _ if !server.is_empty() => server,
        _ => raw.to_string(),
    }
}

/// The local folder for a connected URI, if gvfs exposes one.
pub fn folder_for(uri: &str) -> Option<PathBuf> {
    let host = host_of(uri);
    let share = share_of(uri).map(|s| s.to_lowercase());
    let scheme = uri.split("://").next().unwrap_or("");
    std::fs::read_dir(gvfs_dir()).ok()?.flatten().find_map(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let host_ok = name.contains(&format!("server={host}")) || name.contains(&format!("host={host}"));
        let share_ok = match &share {
            Some(s) if scheme == "smb" => name.contains(&format!("share={s}")),
            _ => true,
        };
        let scheme_ok = name.starts_with(scheme) || (scheme == "smb" && name.starts_with("smb-share"));
        if host_ok && share_ok && scheme_ok {
            // For sftp/ftp the path below the server root still applies.
            let below = if scheme == "smb" {
                uri.split("://").nth(1).unwrap_or("").splitn(3, '/').nth(2).unwrap_or("").to_string()
            } else {
                uri.split("://").nth(1).unwrap_or("").split_once('/').map(|x| x.1.to_string()).unwrap_or_default()
            };
            Some(e.path().join(below.trim_matches('/')))
        } else {
            None
        }
    })
}

/// Connect a location. Answers gio's prompts from `login`; with no login, a prompt
/// for a user name or password ends the attempt with `NeedsLogin`.
pub fn mount(uri: &str, login: Option<&Login>) -> Result<(), MountError> {
    let mut cmd = Command::new("gio");
    cmd.arg("mount");
    if login.is_some_and(|l| l.anonymous) {
        cmd.arg("-a");
    }
    let mut child = cmd
        .arg(uri)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| MountError::Failed(format!("gio is not available: {e}")))?;
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take().unwrap();
    // Read prompts on a helper thread so we can time out.
    let (tx, rx) = std::sync::mpsc::channel::<u8>();
    std::thread::spawn(move || {
        let mut b = [0u8; 1];
        while let Ok(1) = stdout.read(&mut b) {
            if tx.send(b[0]).is_err() {
                break;
            }
        }
    });
    let start = Instant::now();
    let mut buf = String::new();
    let mut answered = 0;
    loop {
        while let Ok(c) = rx.try_recv() {
            buf.push(c as char);
        }
        // A prompt ends in ": " (gio's own prompts) and has no newline after it.
        if buf.ends_with(": ") || buf.ends_with("? ") {
            let prompt = buf.rsplit('\n').next().unwrap_or("").to_lowercase();
            buf.clear();
            let answer = match login {
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(MountError::NeedsLogin);
                }
                Some(l) if prompt.contains("user") => l.user.clone(),
                Some(l) if prompt.contains("domain") => l.domain.clone(),
                Some(l) if prompt.contains("password") => l.password.clone(),
                Some(_) => "1".into(), // e.g. "choose an option" for host keys: accept the first
            };
            answered += 1;
            if answered > 6 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MountError::Failed("The server keeps asking for a login — check the user name and password.".into()));
            }
            if let Some(s) = stdin.as_mut() {
                let _ = s.write_all(format!("{answer}\n").as_bytes());
                let _ = s.flush();
            }
        }
        if let Some(status) = child.try_wait().map_err(|e| MountError::Failed(e.to_string()))? {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            if status.success() {
                return Ok(());
            }
            let msg = err.lines().last().unwrap_or("").trim().to_string();
            let msg = msg.split(": ").skip(1).collect::<Vec<_>>().join(": ");
            let msg = if msg.is_empty() { "Could not connect.".to_string() } else { msg };
            // Already connected counts as success.
            if msg.to_lowercase().contains("already mounted") {
                return Ok(());
            }
            if login.is_none() && (msg.to_lowercase().contains("password") || msg.to_lowercase().contains("permission")) {
                return Err(MountError::NeedsLogin);
            }
            return Err(MountError::Failed(msg));
        }
        if start.elapsed() > Duration::from_secs(25) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MountError::Failed("The server did not answer in time.".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Disconnect a connected folder (/run/user/…/gvfs/<name>).
pub fn unmount_folder(folder: &std::path::Path) -> Result<(), String> {
    let ok = Command::new("gio")
        .args(["mount", "-u"])
        .arg(folder)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok { Ok(()) } else { Err("Could not disconnect (is something still using it?)".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_server_listed_once() {
        let p = |n: &str, u: &str| NetPlace { name: n.into(), uri: u.into() };
        let v = dedupe(vec![p("WDMYCLOUD", "smb://WDMYCLOUD/"), p("WDMyCloud", "smb://WDMyCloud.local/"), p("WDMyCloud", "sftp://WDMyCloud.local/")]);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "WDMyCloud");
    }

    /// A server that does not exist gives a plain error, not a hang.
    #[test]
    fn unknown_server_fails() {
        if std::process::Command::new("gio").arg("--version").output().is_err() {
            return;
        }
        let t = std::time::Instant::now();
        let r = mount("sftp://rivet-no-such-host.invalid/", Some(&Login { anonymous: true, ..Default::default() }));
        assert!(matches!(r, Err(MountError::Failed(_))), "{:?}", r.err().map(|e| match e { MountError::Failed(s) => s, MountError::NeedsLogin => "login".into() }));
        assert!(t.elapsed().as_secs() < 30);
    }

    #[test]
    fn parses_network_listing() {
        let t = "dnssd-server-NAS._smb._tcp\t0\t(shortcut)\tstandard::display-name=My NAS (File Sharing) standard::icon=GThemedIcon:0x1 standard::target-uri=smb://NAS.local:445/\n\
                 public\t0\t(directory)\tstandard::display-name=public\n\
                 IPC$\t0\t(directory)\tstandard::display-name=IPC$\n";
        let v = parse_gio_list(t, Some("smb://NAS.local:445"));
        assert_eq!(v[0], NetPlace { name: "My NAS (File Sharing)".into(), uri: "smb://NAS.local:445/".into() });
        assert_eq!(v[1].uri, "smb://NAS.local:445/public/");
        assert_eq!(v.len(), 2, "hidden IPC$ share is skipped");
        assert!(v[0].lists_shares() && !v[1].lists_shares());
    }

    #[test]
    fn names_and_hosts() {
        assert_eq!(host_of("smb://user@NAS.local:445/public/x"), "nas.local");
        assert_eq!(share_of("smb://nas.local/public/x"), Some("public".into()));
        assert_eq!(share_of("smb://nas.local/"), None);
        assert_eq!(pretty_mount_name("smb-share:server=nas.local,share=public"), "public on nas.local");
        assert_eq!(pretty_mount_name("sftp:host=zima.local"), "zima.local (SFTP)");
    }
}
