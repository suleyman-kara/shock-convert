//! Basit dosya günlüğü. Sağ tıktan başlatılan süreçte konsol olmadığı için
//! hatalar yalnızca burada görülebilir: `%LOCALAPPDATA%\ShockConvert\log.txt`.

use std::io::Write;
use std::path::PathBuf;

const MAX_BYTES: u64 = 256 * 1024;

pub fn path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ShockConvert").join("log.txt")
}

pub fn line(msg: &str) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Dosya sonsuza kadar büyümesin.
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::remove_file(&p);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
    {
        let _ = writeln!(f, "[{secs}] pid={} {msg}", std::process::id());
    }
}
