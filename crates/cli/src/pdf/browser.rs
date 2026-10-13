//! HTML -> PDF: kurulu Edge/Chrome/Chromium'u başsız (headless) çalıştırıp "PDF olarak yazdır" yapar.
//! Windows 10/11'de Edge her zaman vardır, bu yüzden ek kurulum gerekmez.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::process::{self, TempDir, file_url};

pub const HINT: &str = "PDF için Microsoft Edge veya Google Chrome gerekli ama bulunamadı. \
     presets.toml içinde `browser_path` ile yolunu verebilirsiniz.";

const TIMEOUT: Duration = Duration::from_secs(120);

/// Sırayla: yapılandırılmış yol, `SHOCK_CONVERT_BROWSER`, Windows'ta Edge/Chrome kurulum yolları, `PATH`.
pub fn locate(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = configured.filter(|p| p.is_file()) {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("SHOCK_CONVERT_BROWSER")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
            candidates.push(base.join(r"Microsoft\Edge\Application\msedge.exe"));
            candidates.push(base.join(r"Google\Chrome\Application\chrome.exe"));
        }
    }
    if let Some(c) = candidates.into_iter().find(|c| c.is_file()) {
        return Some(c);
    }
    let names = [
        "msedge",
        "microsoft-edge",
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ];
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        names.iter().find_map(|n| {
            let exe = dir.join(if cfg!(windows) {
                format!("{n}.exe")
            } else {
                (*n).to_string()
            });
            exe.is_file().then_some(exe)
        })
    })
}

/// `html` dosyasını (ya da `file://` ile açılabilen herhangi bir yerel sayfayı) `out_pdf`'e basar.
pub fn html_to_pdf(browser: &Path, html: &Path, out_pdf: &Path) -> Result<(), String> {
    // Çalışan bir Edge/Chrome örneğine devredip hemen çıkmasın diye ayrı, tek kullanımlık profil.
    let profile = TempDir::new("browser").map_err(|e| e.to_string())?;
    let mut cmd = Command::new(browser);
    cmd.arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--no-pdf-header-footer")
        .arg("--print-to-pdf-no-header")
        .arg(format!("--user-data-dir={}", profile.0.display()))
        .arg(format!("--print-to-pdf={}", out_pdf.display()));
    if std::env::var_os("SHOCK_CONVERT_NO_SANDBOX").is_some() {
        cmd.arg("--no-sandbox"); // yalnızca kök kullanıcı/konteyner testleri için
    }
    cmd.arg(file_url(html));

    let ok = process::run(&mut cmd, TIMEOUT).map_err(|e| format!("tarayıcı: {e}"))?;
    let produced = std::fs::metadata(out_pdf).is_ok_and(|m| m.len() > 0);
    if produced {
        return Ok(()); // bazı sürümler PDF yazdıktan sonra sıfırdan farklı kodla çıkar
    }
    Err(if ok {
        "tarayıcı PDF üretmedi".into()
    } else {
        "tarayıcı hata verdi ve PDF üretmedi".into()
    })
}
