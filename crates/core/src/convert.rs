use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageReader, Rgba, RgbaImage};

use crate::format::Format;
use crate::output::unique_output_path;

const JPEG_QUALITY: u8 = 90;
const ICO_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("dosya açılamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("görüntü işlenemedi: {0}")]
    Image(#[from] image::ImageError),
}

/// `input` dosyasını `target` formatına çevirip yanına `ad-format.format` olarak yazar.
/// Yazılan dosyanın yolunu döndürür. Hata olursa yarım kalan çıktı silinir.
pub fn convert_file(input: &Path, target: Format) -> Result<PathBuf, ConvertError> {
    let img = load(input)?;
    let out = unique_output_path(input, target.ext(), target.ext())?;
    match write(&img, &out, target) {
        Ok(()) => Ok(out),
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            Err(e)
        }
    }
}

/// Dosyayı açar ve EXIF yönünü uygular. Animasyonlu girdilerde ilk kare alınır.
fn load(input: &Path) -> Result<DynamicImage, ConvertError> {
    let reader = ImageReader::open(input)?.with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

fn write(img: &DynamicImage, out: &Path, target: Format) -> Result<(), ConvertError> {
    match target {
        Format::Jpg => {
            let rgb = flatten_on_white(img);
            let w = BufWriter::new(File::create(out)?);
            JpegEncoder::new_with_quality(w, JPEG_QUALITY).encode_image(&rgb)?;
        }
        Format::Ico => write_ico(img, out)?,
        Format::Gif | Format::Bmp => {
            // Bu kodlayıcılar yalnızca 8 bit RGBA/RGB kabul eder.
            DynamicImage::ImageRgba8(img.to_rgba8())
                .save_with_format(out, target.image_format())?;
        }
        _ => img.save_with_format(out, target.image_format())?,
    }
    Ok(())
}

/// Şeffaf alanı beyaza oturtur (JPG şeffaflık desteklemez).
fn flatten_on_white(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let mut bg = RgbaImage::from_pixel(rgba.width(), rgba.height(), Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut bg, &rgba, 0, 0);
    DynamicImage::ImageRgba8(bg).to_rgb8().into()
}

/// 16–256 px arası çoklu boyutlu ICO üretir; kaynaktan büyük boyutlar eklenmez.
fn write_ico(img: &DynamicImage, out: &Path) -> Result<(), ConvertError> {
    let longest = img.width().max(img.height());
    let mut sizes: Vec<u32> = ICO_SIZES
        .iter()
        .copied()
        .filter(|&s| s <= longest)
        .collect();
    if sizes.is_empty() {
        sizes.push(longest.max(1));
    }

    let frames = sizes
        .iter()
        .map(|&s| {
            // En-boy oranını koruyup s×s şeffaf tuvale ortalar.
            let scaled = img.resize(s, s, FilterType::Lanczos3).to_rgba8();
            let mut canvas = RgbaImage::new(s, s);
            let x = (s - scaled.width()) / 2;
            let y = (s - scaled.height()) / 2;
            image::imageops::overlay(&mut canvas, &scaled, x as i64, y as i64);
            IcoFrame::as_png(canvas.as_raw(), s, s, ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let w = BufWriter::new(File::create(out)?);
    IcoEncoder::new(w).encode_images(&frames)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_png(dir: &Path) -> PathBuf {
        let mut img = RgbaImage::from_pixel(64, 48, Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0])); // şeffaf piksel
        let p = dir.join("kare.png");
        img.save(&p).unwrap();
        p
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shock-convert-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn converts_to_every_format_and_output_is_readable() {
        let d = temp_dir("convert");
        let input = sample_png(&d);
        for target in Format::ALL {
            if target == Format::Png {
                continue;
            }
            let out = convert_file(&input, target).unwrap();
            assert_eq!(
                out.file_name().unwrap().to_string_lossy(),
                format!("kare-{}.{}", target.ext(), target.ext())
            );
            let back = image::open(&out).unwrap_or_else(|e| panic!("{target:?}: {e}"));
            assert!(back.width() > 0);
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn jpg_flattens_transparency_to_white() {
        let d = temp_dir("jpg");
        let input = d.join("seffaf.png");
        RgbaImage::new(32, 32).save(&input).unwrap(); // tamamen şeffaf
        let out = convert_file(&input, Format::Jpg).unwrap();
        let px = image::open(out).unwrap().to_rgb8().get_pixel(16, 16).0;
        assert!(
            px.iter().all(|&c| c > 240),
            "şeffaf piksel beyaz olmalı: {px:?}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn corrupt_file_errors_and_leaves_no_output() {
        let d = temp_dir("corrupt");
        let bad = d.join("bozuk.png");
        fs::write(&bad, b"bu bir png degil").unwrap();
        assert!(convert_file(&bad, Format::Jpg).is_err());
        assert!(!d.join("bozuk-jpg.jpg").exists());
        let _ = fs::remove_dir_all(&d);
    }
}
