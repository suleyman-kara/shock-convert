//! PDF'e dönüştürme. Girdi türüne göre en uygun, kurulum gerektirmeyen yol seçilir:
//!
//! | Girdi | Yol | Gereken |
//! |---|---|---|
//! | Görüntüler | kendi PDF yazıcımız (çekirdek) | -- |
//! | TXT, MD, HTML | tarayıcı motoru ("PDF olarak yazdır") | Edge/Chrome (Windows'ta hazır) |
//! | DOCX, XLSX, PPTX | yerleşik çözümleyici -> HTML -> tarayıcı; LibreOffice varsa o tercih edilir | -- (isteğe bağlı LibreOffice) |
//! | DOC, XLS, PPT, RTF, ODT, ODS, ODP | LibreOffice | LibreOffice |

mod browser;
mod docx;
#[cfg(test)]
mod fixtures;
mod html;
mod office;
mod ooxml;
mod pptx;
mod process;
mod text;
mod xlsx;
mod xml;

use std::path::{Path, PathBuf};

use shock_convert_core::{Format, image_to_pdf, unique_output_path};

use self::process::TempDir;

/// Office belgelerinde hangi motorun kullanılacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Engine {
    /// LibreOffice varsa onu, yoksa yerleşik dönüştürücüyü kullan.
    #[default]
    Auto,
    /// Her zaman yerleşik dönüştürücü (hızlı, orta doğruluk).
    Builtin,
    /// Yalnızca LibreOffice (yoksa hata).
    LibreOffice,
}

impl Engine {
    pub fn parse(s: &str) -> Option<Engine> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Engine::Auto),
            "builtin" => Some(Engine::Builtin),
            "libreoffice" => Some(Engine::LibreOffice),
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
        Class::Ooxml(kind) => {
            let office = office::locate(job.libreoffice_path.as_deref());
            match (job.engine, office) {
                (Engine::Builtin, _) | (Engine::Auto, None) => builtin_ooxml(file, kind, job),
                (_, Some(soffice)) => finalize(file, |out| office::to_pdf(&soffice, file, out)),
                (Engine::LibreOffice, None) => Err(office::HINT.into()),
            }
        }
        Class::OfficeOnly => {
            let soffice = office::locate(job.libreoffice_path.as_deref()).ok_or(office::HINT)?;
            finalize(file, |out| office::to_pdf(&soffice, file, out))
        }
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
        assert_eq!(Engine::parse("x"), None);
    }
}
