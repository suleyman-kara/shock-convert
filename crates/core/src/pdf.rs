//! Görüntüden PDF: harici program olmadan, tek sayfa, sayfa görüntü boyutundadır.
//! Baseline JPG'ler yeniden sıkıştırılmadan (DCTDecode) gömülür; diğerleri kayıpsız (Flate) gömülür.

use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::ZlibEncoder;
use image::{ColorType, ImageDecoder, ImageFormat, ImageReader};

use crate::convert::{ConvertError, flatten_on, load};
use crate::output::unique_output_path;

/// 96 dpi'lık piksel -> PDF noktası (1/72 inç).
const PX_TO_PT: f64 = 0.75;
/// PDF 1.4 sayfa kenarı üst sınırı (birim).
const MAX_PAGE_PT: f64 = 14_400.0;

/// `input` görüntüsünü yanına aynı adla `.pdf` olarak yazar; yolunu döndürür.
/// Hata olursa yarım çıktı silinir.
pub fn image_to_pdf(input: &Path) -> Result<PathBuf, ConvertError> {
    let page = embed(input)?;
    let out = unique_output_path(input, None, "pdf")?;
    match std::fs::write(&out, build_pdf(&page)) {
        Ok(()) => Ok(out),
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            Err(e.into())
        }
    }
}

struct Page {
    width: u32,
    height: u32,
    /// PDF `/ColorSpace` adı.
    color_space: &'static str,
    /// PDF `/Filter` adı.
    filter: &'static str,
    data: Vec<u8>,
}

fn embed(input: &Path) -> Result<Page, ConvertError> {
    if let Some(p) = jpeg_passthrough(input)? {
        return Ok(p);
    }
    let img = load(input)?;
    let rgb = flatten_on(&img, [255, 255, 255]).to_rgb8();
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(rgb.as_raw())?;
    Ok(Page {
        width: rgb.width(),
        height: rgb.height(),
        color_space: "/DeviceRGB",
        filter: "/FlateDecode",
        data: z.finish()?,
    })
}

/// JPG yönü düzgünse ve RGB/gri ise özgün baytlar doğrudan gömülür (kalite kaybı ve şişme yok).
fn jpeg_passthrough(input: &Path) -> Result<Option<Page>, ConvertError> {
    let reader = ImageReader::open(input)?.with_guessed_format()?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Ok(None);
    }
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    let color_space = match decoder.color_type() {
        ColorType::Rgb8 => "/DeviceRGB",
        ColorType::L8 => "/DeviceGray",
        _ => return Ok(None),
    };
    if decoder.orientation()? != image::metadata::Orientation::NoTransforms {
        return Ok(None); // yön düzeltmesi gerekiyor: yeniden kodlayarak gömülür
    }
    Ok(Some(Page {
        width,
        height,
        color_space,
        filter: "/DCTDecode",
        data: std::fs::read(input)?,
    }))
}

fn build_pdf(p: &Page) -> Vec<u8> {
    let scale = PX_TO_PT.min(MAX_PAGE_PT / f64::from(p.width.max(p.height).max(1)));
    let (w, h) = (f64::from(p.width) * scale, f64::from(p.height) * scale);

    let content = format!("q {w:.2} 0 0 {h:.2} 0 0 cm /Im0 Do Q");
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();

    let mut obj = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    obj(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(&mut out, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    obj(
        &mut out,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] \
             /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>"
        )
        .as_bytes(),
    );

    let mut image = format!(
        "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} \
         /BitsPerComponent 8 /Filter {} /Length {} >>\nstream\n",
        p.width,
        p.height,
        p.color_space,
        p.filter,
        p.data.len()
    )
    .into_bytes();
    image.extend_from_slice(&p.data);
    image.extend_from_slice(b"\nendstream");
    obj(&mut out, &image);

    obj(
        &mut out,
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
        .as_bytes(),
    );

    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("shock-convert-pdf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// `xref` tablosundaki her ofset gerçekten "<n> 0 obj" satırına işaret etmeli.
    fn assert_structurally_valid(pdf: &[u8]) {
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(pdf);
        let start: usize = text
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[start..].starts_with(b"xref"));
        let table = &pdf[start..];
        let lines: Vec<&[u8]> = table.split(|&b| b == b'\n').collect();
        let n: usize = String::from_utf8_lossy(lines[1])
            .split(' ')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        for i in 1..n {
            let off: usize = String::from_utf8_lossy(lines[2 + i])[..10].parse().unwrap();
            assert!(
                pdf[off..].starts_with(format!("{i} 0 obj").as_bytes()),
                "nesne {i}"
            );
        }
    }

    #[test]
    fn png_becomes_flate_pdf_named_after_input() {
        let d = temp_dir("png");
        let input = d.join("rapor.png");
        RgbaImage::from_pixel(40, 20, Rgba([200, 30, 30, 255]))
            .save(&input)
            .unwrap();
        let out = image_to_pdf(&input).unwrap();
        assert_eq!(out, d.join("rapor.pdf"));
        let pdf = std::fs::read(&out).unwrap();
        assert_structurally_valid(&pdf);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/FlateDecode") && text.contains("/MediaBox [0 0 30.00 15.00]"));
        // İkinci çağrı üzerine yazmaz.
        assert_eq!(image_to_pdf(&input).unwrap(), d.join("rapor (1).pdf"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn baseline_jpeg_is_embedded_byte_for_byte() {
        let d = temp_dir("jpg");
        let png = d.join("foto.png");
        RgbaImage::from_pixel(64, 48, Rgba([10, 120, 200, 255]))
            .save(&png)
            .unwrap();
        let jpg = crate::convert_file(&png, crate::Format::Jpg).unwrap();
        let original = std::fs::read(&jpg).unwrap();

        let pdf = std::fs::read(image_to_pdf(&jpg).unwrap()).unwrap();
        assert_structurally_valid(&pdf);
        assert!(String::from_utf8_lossy(&pdf).contains("/DCTDecode"));
        assert!(
            pdf.windows(original.len()).any(|w| w == original),
            "özgün JPG baytları gömülmeli"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn huge_images_are_scaled_to_pdf_page_limit() {
        let page = Page {
            width: 30_000,
            height: 10,
            color_space: "/DeviceGray",
            filter: "/FlateDecode",
            data: vec![0],
        };
        let text = String::from_utf8_lossy(&build_pdf(&page)).into_owned();
        assert!(text.contains("/MediaBox [0 0 14400.00 4.80]"), "{text}");
    }

    #[test]
    fn corrupt_image_errors_without_leaving_pdf() {
        let d = temp_dir("bad");
        let bad = d.join("bozuk.png");
        std::fs::write(&bad, b"png degil").unwrap();
        assert!(image_to_pdf(&bad).is_err());
        assert!(!d.join("bozuk.pdf").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
