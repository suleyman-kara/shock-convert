//! Harici programı zaman aşımıyla çalıştırır (takılan tarayıcı/LibreOffice süreci sonsuza dek beklemesin).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Çıkış kodunu değil, programın bitip bitmediğini döndürür: `Ok(true)` başarıyla çıktı,
/// `Ok(false)` sıfırdan farklı kodla çıktı, `Err` başlatılamadı ya da zaman aşımına uğradı.
pub fn run(cmd: &mut Command, limit: Duration) -> Result<bool, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("başlatılamadı: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) if crate::job::cancelled() => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(crate::job::CANCELLED.into());
            }
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{} sn içinde bitmedi, durduruldu", limit.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Her çağrı için benzersiz geçici çalışma klasörü (kapanışta silinir).
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> std::io::Result<TempDir> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "shock-convert-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)?;
        Ok(TempDir(dir))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `file:///` URL'si; boşluk ve Unicode yüzde-kodlanır, Windows ters eğik çizgileri düzeltilir.
pub fn file_url(path: &std::path::Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("//?/");
    let mut out = String::from("file:///");
    for b in s.trim_start_matches('/').bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_encodes_spaces_and_unicode() {
        let url = file_url(std::path::Path::new("/tmp/Rapor Şubat.html"));
        assert_eq!(url, "file:///tmp/Rapor%20%C5%9Eubat.html");
    }

    #[test]
    fn timeout_kills_hanging_process() {
        if cfg!(windows) {
            return;
        }
        let err = run(Command::new("sleep").arg("30"), Duration::from_millis(300)).unwrap_err();
        assert!(err.contains("durduruldu"), "{err}");
        assert_eq!(
            run(&mut Command::new("true"), Duration::from_secs(5)),
            Ok(true)
        );
        assert_eq!(
            run(&mut Command::new("false"), Duration::from_secs(5)),
            Ok(false)
        );
    }
}
