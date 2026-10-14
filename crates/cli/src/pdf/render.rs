//! PDFium ile sayfa çizen işlemler: PDF → görüntü (PNG/JPG) ve PDF → metin.
//!
//! PDFium (BSD/Apache-2.0, Chromium'un PDF motoru) ayrı bir kitaplıktır: Windows'ta kurulum
//! klasöründe exe'nin yanındaki `pdfium.dll`. Sürüm `pdfium-render` özelliğiyle eşleşmeli
//! (bkz. `third_party/pdfium.txt`). Bulunamazsa yalnızca bu işlemler anlaşılır bir hatayla durur.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use pdfium_render::prelude::*;
use shock_convert_core::{Format, Options, reserve_output, save_image, write_output};

const HINT: &str =
    "PDFium kitaplığı (pdfium.dll) bulunamadı; Shock Convert'i yeniden kurmak sorunu çözer";

/// Çok büyük sayfalarda bellek taşmasın: bir kenar en fazla bu kadar piksel.
const MAX_SIDE: i32 = 12_000;

fn library_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // Testler ve geliştirme için: kitaplığın kendisi ya da bulunduğu klasör.
    if let Some(p) = std::env::var_os("SHOCK_CONVERT_PDFIUM").map(PathBuf::from) {
        if p.is_dir() {
            v.push(Pdfium::pdfium_platform_library_name_at_path(&p));
        } else {
            v.push(p);
        }
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        v.push(Pdfium::pdfium_platform_library_name_at_path(&dir));
    }
    v
}

/// PDFium süreç başına bir kez yüklenir; `thread_safe` özelliği çağrıları sıraya koyar.
fn pdfium() -> Result<&'static Pdfium, String> {
    static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    PDFIUM
        .get_or_init(|| {
            let mut last = String::new();
            for path in library_candidates().into_iter().filter(|p| p.is_file()) {
                match Pdfium::bind_to_library(&path) {
                    Ok(b) => return Ok(Pdfium::new(b)),
                    Err(e) => last = format!("{}: {e}", path.display()),
                }
            }
            match Pdfium::bind_to_system_library() {
                Ok(b) => Ok(Pdfium::new(b)),
                Err(_) if !last.is_empty() => Err(format!("{HINT} ({last})")),
                Err(_) => Err(HINT.into()),
            }
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn open<'a>(pdfium: &'a Pdfium, file: &Path) -> Result<PdfDocument<'a>, String> {
    pdfium.load_pdf_from_file(file, None).map_err(|e| match e {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            "parola korumalı PDF'ler desteklenmiyor".to_string()
        }
        e => format!("PDF okunamadı: {e}"),
    })
}

/// Her sayfayı `dpi` çözünürlükte görüntüye çizer. Tek sayfalık belgede `ad.png`, çok sayfalıda
/// `ad-<ek>-01.png`, `-02` … Yazılanları sırayla döndürür.
pub fn to_images(
    file: &Path,
    target: Format,
    dpi: u32,
    suffix: &str,
    opts: &Options,
) -> Result<Vec<PathBuf>, String> {
    let pdfium = pdfium()?;
    let doc = open(pdfium, file)?;
    let pages = doc.pages();
    let count = pages.len() as usize;
    if count == 0 {
        return Err("PDF'te sayfa yok".into());
    }
    let width = count.to_string().len();
    let scale = dpi.clamp(36, 600) as f32 / 72.0;
    let config = PdfRenderConfig::new()
        .scale_page_by_factor(scale)
        .set_maximum_width(MAX_SIDE)
        .set_maximum_height(MAX_SIDE)
        .set_clear_color(PdfColor::WHITE)
        .render_form_data(true)
        .render_annotations(true);

    let mut outputs = Vec::with_capacity(count);
    for (i, page) in pages.iter().enumerate() {
        let img = page
            .render_with_config(&config)
            .and_then(|b| b.as_image())
            .map_err(|e| format!("{}. sayfa çizilemedi: {e}", i + 1))?;
        // Sayfa beyaz zemine çizildi; saydamlık kanalı yalnızca dosyayı büyütür.
        let img = shock_convert_core::image::DynamicImage::ImageRgb8(img.to_rgb8());
        let reserved = if count == 1 {
            reserve_output(file, None, target.ext())
        } else {
            reserve_output(
                file,
                Some(&format!("{suffix}-{:0width$}", i + 1)),
                target.ext(),
            )
        }
        .map_err(|e| e.to_string())?;
        save_image(&img, reserved.path(), target, opts).map_err(|e| e.to_string())?;
        outputs.push(reserved.commit().map_err(|e| e.to_string())?);
    }
    Ok(outputs)
}

/// Sayfaların metnini okuma sırasıyla `ad.txt` (UTF-8) olarak yazar; sayfalar boş satırla ayrılır.
pub fn to_text(file: &Path) -> Result<PathBuf, String> {
    let pdfium = pdfium()?;
    let doc = open(pdfium, file)?;
    let mut text = String::new();
    for (i, page) in doc.pages().iter().enumerate() {
        let page_text = page
            .text()
            .map_err(|e| format!("{}. sayfanın metni okunamadı: {e}", i + 1))?
            .all();
        if i > 0 {
            text.push_str("\r\n\r\n");
        }
        // PDFium satırları \r\n ile verir; tek \n kalırsa Windows araçlarında da düzgün görünsün.
        text.push_str(
            page_text
                .replace("\r\n", "\n")
                .replace('\n', "\r\n")
                .trim_end(),
        );
    }
    if text.trim().is_empty() {
        return Err(
            "PDF'te seçilebilir metin yok (taranmış bir belge olabilir; metin tanıma desteklenmiyor)"
                .into(),
        );
    }
    text.push_str("\r\n");
    write_output(file, None, "txt", text.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn available() -> bool {
    pdfium().is_ok()
}
