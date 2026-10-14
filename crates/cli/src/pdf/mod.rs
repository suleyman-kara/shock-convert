//! PDF'e dönüştürme. Girdi türüne göre en uygun, kurulum gerektirmeyen yol seçilir:
//!
//! | Girdi | Yol | Gereken |
//! |---|---|---|
//! | Görüntüler | kendi PDF yazıcımız (çekirdek) | -- |
//! | TXT, MD, HTML | tarayıcı motoru ("PDF olarak yazdır") | Edge/Chrome (Windows'ta hazır) |
//! | DOCX, XLSX, PPTX | Microsoft Office -> LibreOffice -> yerleşik çözümleyici (HTML -> tarayıcı) | -- (Office/LibreOffice varsa onlar) |
//! | DOC, XLS, PPT, RTF, ODT, ODS, ODP | Microsoft Office -> LibreOffice | Office ya da LibreOffice |
//!
//! Office belgelerinde bir motor başarısız olursa (ör. Office etkinleştirilmemiş) sıradakine geçilir.

mod browser;
mod docx;
#[cfg(test)]
mod fixtures;
mod html;
mod libreoffice;
mod msoffice;
mod ooxml;
mod pptx;
mod process;
mod text;
pub mod tools;
mod xlsx;
mod xml;

use std::path::{Path, PathBuf};

use shock_convert_core::{Format, image_to_pdf, unique_output_path};

use self::process::TempDir;

/// Office belgelerinde hangi motorun kullanılacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Engine {
    /// Sırayla Microsoft Office, LibreOffice, yerleşik dönüştürücü; olmayan ya da başarısız
    /// olan atlanır.
    #[default]
    Auto,
    /// Her zaman yerleşik dönüştürücü (hızlı, orta doğruluk).
    Builtin,
    /// Yalnızca LibreOffice (yoksa hata).
    LibreOffice,
    /// Yalnızca Microsoft Office (yoksa hata).
    Office,
}

impl Engine {
    pub fn parse(s: &str) -> Option<Engine> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Engine::Auto),
            "builtin" => Some(Engine::Builtin),
            "libreoffice" => Some(Engine::LibreOffice),
            "office" | "msoffice" => Some(Engine::Office),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PdfJob {
    pub browser_path: Option<PathBuf>,
    pub libreoffice_path: Option<PathBuf>,
    pub engine: Engine,
}

#[derive(Debug, PartialEq, Eq)]
enum Class {
    Image,
    Plain,
    Markdown,
    Html,
    /// Yerleşik çözümleyicisi olan OOXML belgeleri.
    Ooxml(Ooxml),
    /// Yalnızca LibreOffice'in açabildiği türler.
    OfficeOnly,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Ooxml {
    Docx,
    Xlsx,
    Pptx,
}

fn classify(ext: &str) -> Class {
    let ext = ext.to_ascii_lowercase();
    if Format::from_ext(&ext).is_some() {
        return Class::Image;
    }
    match ext.as_str() {
        "txt" | "log" => Class::Plain,
        "md" | "markdown" => Class::Markdown,
        "html" | "htm" => Class::Html,
        "docx" => Class::Ooxml(Ooxml::Docx),
        "xlsx" => Class::Ooxml(Ooxml::Xlsx),
        "pptx" => Class::Ooxml(Ooxml::Pptx),
        "doc" | "xls" | "ppt" | "rtf" | "odt" | "ods" | "odp" => Class::OfficeOnly,
        _ => Class::Unsupported,
    }
}

/// Başlamadan "dönüştürülüyor…" bildirimi gösterilmeli mi (görüntü→PDF anlıktır).
pub fn is_slow(ext: &str) -> bool {
    !matches!(classify(ext), Class::Image)
}

pub fn run(file: &Path, job: &PdfJob) -> Result<PathBuf, String> {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    match classify(&ext) {
        Class::Image => image_to_pdf(file).map_err(|e| e.to_string()),
        Class::Plain => via_browser(file, job, |bytes, title| text::plain_to_html(title, bytes)),
        Class::Markdown => via_browser(file, job, |bytes, title| {
            text::markdown_to_html(title, bytes)
        }),
        Class::Html => html_file(file, job),
        Class::Ooxml(kind) => office_document(file, &ext, Some(kind), job),
        Class::OfficeOnly => office_document(file, &ext, None, job),
        Class::Unsupported => Err(format!("`.{ext}` PDF'e dönüştürülemez")),
    }
}

/// Çıktı adını rezerve eder (`ad.pdf`, doluysa `ad (1).pdf`), `produce` içine yazdırır; hata olursa siler.
fn finalize(
    input: &Path,
    produce: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let out = unique_output_path(input, None, "pdf").map_err(|e| e.to_string())?;
    match produce(&out) {
        Ok(()) if std::fs::metadata(&out).is_ok_and(|m| m.len() > 0) => Ok(out),
        other => {
            let _ = std::fs::remove_file(&out);
            Err(other.err().unwrap_or_else(|| "boş PDF üretildi".into()))
        }
    }
}

fn browser_for(job: &PdfJob) -> Result<PathBuf, String> {
    browser::locate(job.browser_path.as_deref()).ok_or_else(|| browser::HINT.to_string())
}

/// Dosya içeriğini `to_html` ile HTML'e çevirip tarayıcıyla PDF'e basar.
fn via_browser(
    file: &Path,
    job: &PdfJob,
    to_html: impl FnOnce(&[u8], &str) -> String,
) -> Result<PathBuf, String> {
    let browser = browser_for(job)?;
    let bytes = std::fs::read(file).map_err(|e| format!("dosya okunamadı: {e}"))?;
    let title = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    html_to_pdf(file, &browser, &to_html(&bytes, &title))
}

fn html_to_pdf(input: &Path, browser: &Path, html: &str) -> Result<PathBuf, String> {
    let work = TempDir::new("html").map_err(|e| e.to_string())?;
    let page = work.0.join("page.html");
    std::fs::write(&page, html).map_err(|e| e.to_string())?;
    finalize(input, |out| browser::html_to_pdf(browser, &page, out))
}

/// HTML dosyası doğrudan açılır; böylece yanındaki resim/CSS göreli yollarla yüklenir.
fn html_file(file: &Path, job: &PdfJob) -> Result<PathBuf, String> {
    let browser = browser_for(job)?;
    finalize(file, |out| browser::html_to_pdf(&browser, file, out))
}

const NO_OFFICE_APP: &str = "Bu belge türü için Microsoft Office ya da LibreOffice gerekli ama ikisi de \
     bulunamadı. LibreOffice ücretsizdir: winget install TheDocumentFoundation.LibreOffice";

/// Office belgesini motor sırasıyla dener: Microsoft Office -> LibreOffice -> yerleşik.
/// `builtin` yalnızca yerleşik çözümleyicisi olan türlerde (DOCX/XLSX/PPTX) verilir.
fn office_document(
    file: &Path,
    ext: &str,
    builtin: Option<Ooxml>,
    job: &PdfJob,
) -> Result<PathBuf, String> {
    let engine = match (job.engine, builtin) {
        // Yerleşik dönüştürücünün açamadığı türde "builtin" seçimi otomatik sıraya düşer.
        (Engine::Builtin, None) => Engine::Auto,
        (e, _) => e,
    };
    let mut failures: Vec<String> = Vec::new();
    let mut attempt = |name: &str, result: Result<PathBuf, String>| match result {
        Ok(p) => Some(p),
        Err(e) => {
            crate::log::line(&format!("{name} başarısız, sıradaki motor denenecek: {e}"));
            failures.push(e);
            None
        }
    };

    if matches!(engine, Engine::Auto | Engine::Office) {
        match msoffice::App::for_ext(ext).filter(|a| a.installed()) {
            Some(app) => {
                let r = finalize(file, |out| msoffice::to_pdf(app, file, out));
                if let Some(p) = attempt(app.name(), r) {
                    return Ok(p);
                }
            }
            None if engine == Engine::Office => return Err(msoffice::HINT.into()),
            None => {}
        }
    }
    if matches!(engine, Engine::Auto | Engine::LibreOffice) {
        match libreoffice::locate(job.libreoffice_path.as_deref()) {
            Some(soffice) => {
                let r = finalize(file, |out| libreoffice::to_pdf(&soffice, file, out));
                if let Some(p) = attempt("LibreOffice", r) {
                    return Ok(p);
                }
            }
            None if engine == Engine::LibreOffice => return Err(libreoffice::HINT.into()),
            None => {}
        }
    }
    if matches!(engine, Engine::Auto | Engine::Builtin)
        && let Some(kind) = builtin
    {
        return builtin_ooxml(file, kind, job).map_err(|e| {
            failures.push(e);
            failures.join("; ")
        });
    }
    if failures.is_empty() {
        Err(NO_OFFICE_APP.into())
    } else {
        Err(failures.join("; "))
    }
}

fn builtin_ooxml(file: &Path, kind: Ooxml, job: &PdfJob) -> Result<PathBuf, String> {
    let browser = browser_for(job)?;
    let html = match kind {
        Ooxml::Docx => docx::to_html(file)?,
        Ooxml::Xlsx => xlsx::to_html(file)?,
        Ooxml::Pptx => pptx::to_html(file)?,
    };
    html_to_pdf(file, &browser, &html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension() {
        assert_eq!(classify("PNG"), Class::Image);
        assert_eq!(classify("jpeg"), Class::Image);
        assert_eq!(classify("docx"), Class::Ooxml(Ooxml::Docx));
        assert_eq!(classify("DOC"), Class::OfficeOnly);
        assert_eq!(classify("md"), Class::Markdown);
        assert_eq!(classify("exe"), Class::Unsupported);
        assert!(is_slow("docx") && !is_slow("png"));
    }

    #[test]
    fn engine_parses_case_insensitively() {
        assert_eq!(Engine::parse("LibreOffice"), Some(Engine::LibreOffice));
        assert_eq!(Engine::parse("Office"), Some(Engine::Office));
        assert_eq!(Engine::parse("x"), None);
    }
}
