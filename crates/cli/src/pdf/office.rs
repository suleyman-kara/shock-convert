//! LibreOffice ile yüksek doğruluklu dönüştürme (`soffice --headless --convert-to pdf`).
//! İsteğe bağlıdır; yoksa DOCX/XLSX/PPTX için yerleşik dönüştürücü devreye girer.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::process::{self, TempDir, file_url};

pub const HINT: &str = "Bu belge türü için LibreOffice gerekli ama bulunamadı. Kurmak için: \
     winget install TheDocumentFoundation.LibreOffice (presets.toml içinde `libreoffice_path` de verebilirsiniz).";

const TIMEOUT: Duration = Duration::from_secs(300);

pub fn locate(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = configured.filter(|p| p.is_file()) {
        return Some(p.to_path_buf());
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
            candidates.push(base.join(r"LibreOffice\program\soffice.exe"));
        }
    }
    if let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        candidates.push(base.join(r"Programs\LibreOffice\program\soffice.exe"));
    }
    if let Some(c) = candidates.into_iter().find(|c| c.is_file()) {
        return Some(c);
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        ["soffice", "libreoffice"].iter().find_map(|n| {
            let exe = dir.join(if cfg!(windows) {
                format!("{n}.exe")
            } else {
                (*n).to_string()
            });
            exe.is_file().then_some(exe)
        })
    })
}

/// `input`'u PDF'e çevirip `out_pdf`'e yazar.
pub fn to_pdf(soffice: &Path, input: &Path, out_pdf: &Path) -> Result<(), String> {
    let work = TempDir::new("office").map_err(|e| e.to_string())?;
    // Çalışan bir LibreOffice'in kilitlemesine takılmamak için ayrı kullanıcı profili.
    let profile = work.0.join("profile");
    let outdir = work.0.join("out");
    std::fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(soffice);
    cmd.arg("--headless")
        .arg("--norestore")
        .arg("--nolockcheck")
        .arg("--nodefault")
        .arg(format!("-env:UserInstallation={}", file_url(&profile)))
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(&outdir)
        .arg(input);
    process::run(&mut cmd, TIMEOUT).map_err(|e| format!("LibreOffice: {e}"))?;

    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let produced = outdir.join(format!("{stem}.pdf"));
    if !produced.is_file() {
        return Err("LibreOffice PDF üretmedi (dosya bozuk, parola korumalı ya da LibreOffice başka bir işlemde kilitli olabilir)".into());
    }
    std::fs::copy(&produced, out_pdf)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
