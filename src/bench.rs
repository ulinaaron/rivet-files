//! `rivet --bench [FOLDER…]` — time the work behind opening a folder, without a window.
//! Prints only timings and counts (no file names), median of several runs.

use std::path::PathBuf;
use std::time::Instant;

use crate::icons::{self, IconTheme};
use crate::mime::MimeDb;
use crate::model::{Location, Tab};
use crate::settings::{Settings, SortKey};

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

pub fn run(folders: Vec<PathBuf>) {
    const RUNS: usize = 7;
    let folders = if folders.is_empty() { vec![crate::places::home()] } else { folders };

    let t = Instant::now();
    let db = MimeDb::load();
    let mime_ms = ms(t);

    let theme = icons::detect_system_theme();
    let t = Instant::now();
    let mut it = IconTheme::new(&theme);
    let theme_ms = ms(t);

    println!("Rivet benchmark ({} runs each, median)", RUNS);
    println!("  file-type database load      {:>8.1} ms", mime_ms);
    println!("  icon theme index ({theme})  {:>8.1} ms", theme_ms);

    let mut settings = Settings::default();
    for dir in folders {
        let mut tab = Tab::new(Location::Dir(dir.clone()));
        let mut load = Vec::new();
        let mut sort_name = Vec::new();
        let mut sort_type = Vec::new();
        for _ in 0..RUNS {
            let t = Instant::now();
            tab.reload(&db, &settings);
            load.push(ms(t));
            settings.sort = SortKey::Name;
            let t = Instant::now();
            tab.resort(&db, &settings);
            sort_name.push(ms(t));
            settings.sort = SortKey::Type;
            let t = Instant::now();
            tab.resort(&db, &settings);
            sort_type.push(ms(t));
        }
        settings.sort = SortKey::Name;
        // Icon lookup for every item: first pass is cold (disk), second is cached.
        let names: Vec<Vec<String>> = tab.entries.iter().map(|e| db.icon_names(&e.mime)).collect();
        let t = Instant::now();
        let mut found = 0;
        for n in &names {
            let refs: Vec<&str> = n.iter().map(|s| s.as_str()).collect();
            if it.find(&refs, 48).is_some() {
                found += 1;
            }
        }
        let cold = ms(t);
        let t = Instant::now();
        for n in &names {
            let refs: Vec<&str> = n.iter().map(|s| s.as_str()).collect();
            let _ = it.find(&refs, 48);
        }
        let warm = ms(t);
        let n = tab.entries.len();
        println!("\n  folder with {n} items");
        println!("    read + inspect every item    {:>8.1} ms  ({:.1} µs per item)", median(load.clone()), median(load) * 1000.0 / n.max(1) as f64);
        println!("    sort by name                 {:>8.2} ms", median(sort_name));
        println!("    sort by type                 {:>8.2} ms", median(sort_type));
        println!("    find icons, first time       {:>8.1} ms  ({found}/{n} found)", cold);
        println!("    find icons, cached           {:>8.2} ms", warm);
    }
}
