use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::tiff::TiffEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, Rgba, RgbaImage,
};

use crate::format::Format;
use crate::output::unique_output_path;

pub const DEFAULT_JPEG_QUALITY: u8 = 90;
const ICO_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("dosya açılamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("görüntü işlenemedi: {0}")]
    Image(#[from] image::ImageError),
}

/// Dönüştürme ayarları (preset'lerden gelir).
#[derive(Debug, Clone)]
pub struct Options {
    /// JPG kalitesi, 1–100.
    pub jpeg_quality: u8,
    /// Verilirse görüntü bu kutuya sığacak şekilde küçültülür (oran korunur, asla büyütülmez).
    pub max_size: Option<(u32, u32)>,
    /// Şeffaf alanın JPG'de oturtulacağı renk (RGB).
    pub background: [u8; 3],
    /// Dosya adına eklenen ek (`foto-<ek>.<uzantı>`). Boşsa ek yoktur: `foto.png` -> `foto.jpg`.
    pub suffix: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            jpeg_quality: DEFAULT_JPEG_QUALITY,
            max_size: None,
            background: [255, 255, 255],
            suffix: None,
        }
    }
}

/// Varsayılan ayarlarla dönüştürür. Bkz. [`convert_file_with`].
pub fn convert_file(input: &Path, target: Format) -> Result<PathBuf, ConvertError> {
    convert_file_with(input, target, &Options::default())
}

/// `input` dosyasını `target` formatına çevirip yanına `ad-ek.format` olarak yazar.
/// Yazılan dosyanın yolunu döndürür. Hata olursa yarım kalan çıktı silinir.
pub fn convert_file_with(
    input: &Path,
    target: Format,
    opts: &Options,
) -> Result<PathBuf, ConvertError> {
    let (mut img, meta) = load_with_meta(input)?;
    if let Some((mw, mh)) = opts.max_size {
        img = shrink_to_fit(img, mw, mh);
    }
    let out = unique_output_path(input, opts.suffix.as_deref(), target.ext())?;
    match write(&img, &out, target, opts, &meta) {
        Ok(()) => Ok(out),
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            Err(e)
        }
    }
}

/// Bellekteki bir görüntüyü `out` yoluna `target` formatında yazar (ör. PDF sayfası çizimi).
/// Meta veri yoktur; hata olursa yarım kalan dosya silinir.
pub fn save_image(
    img: &DynamicImage,
    out: &Path,
    target: Format,
    opts: &Options,
) -> Result<(), ConvertError> {
    write(img, out, target, opts, &Meta::default()).inspect_err(|_| {
        let _ = std::fs::remove_file(out);
    })
}

/// Dosyayı açar ve EXIF yönünü uygular. Animasyonlu girdilerde ilk kare alınır.
pub(crate) fn load(input: &Path) -> Result<DynamicImage, ConvertError> {
    load_with_meta(input).map(|(img, _)| img)
}

/// Kaynaktan taşınan meta veri: renk profili (ICC) ve EXIF (kamera, tarih, konum …).
#[derive(Debug, Default)]
pub(crate) struct Meta {
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
}

/// Görüntüyü EXIF yönü uygulanmış olarak yükler; ICC ve EXIF'i ayrıca döndürür. Yön pikselere
/// uygulandığı için EXIF'teki yön etiketi "dönüşüm yok" yapılır, yoksa görüntü iki kez dönerdi.
pub(crate) fn load_with_meta(input: &Path) -> Result<(DynamicImage, Meta), ConvertError> {
    let reader = ImageReader::open(input)?.with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    let icc = decoder
        .icc_profile()
        .ok()
        .flatten()
        .filter(|p| !p.is_empty());
    let mut exif = decoder
        .exif_metadata()
        .ok()
        .flatten()
        .filter(|e| !e.is_empty());
    let orientation = decoder.orientation()?;
    if let Some(e) = exif.as_mut() {
        let _ = Orientation::remove_from_exif_chunk(e);
    }
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok((img, Meta { icc, exif }))
}

/// Kodlayıcının desteklediği meta veriyi ekler; desteklemeyenler (ör. TIFF'te EXIF) sessizce atlanır.
fn with_meta<E: ImageEncoder>(mut enc: E, meta: &Meta) -> E {
    if let Some(icc) = &meta.icc {
        let _ = enc.set_icc_profile(icc.clone());
    }
    if let Some(exif) = &meta.exif {
        let _ = enc.set_exif_metadata(exif.clone());
    }
    enc
}

fn write(
    img: &DynamicImage,
    out: &Path,
    target: Format,
    opts: &Options,
    meta: &Meta,
) -> Result<(), ConvertError> {
    let file =
        || -> Result<BufWriter<File>, ConvertError> { Ok(BufWriter::new(File::create(out)?)) };
    match target {
        Format::Jpg => {
            let rgb = flatten_on(img, opts.background);
            let enc = JpegEncoder::new_with_quality(file()?, opts.jpeg_quality.clamp(1, 100));
            rgb.write_with_encoder(with_meta(enc, meta))?;
        }
        Format::Png => img.write_with_encoder(with_meta(PngEncoder::new(file()?), meta))?,
        Format::Webp => {
            // Kayıpsız WebP kodlayıcı yalnızca 8 bit kabul eder.
            let rgba = DynamicImage::ImageRgba8(img.to_rgba8());
            rgba.write_with_encoder(with_meta(WebPEncoder::new_lossless(file()?), meta))?;
        }
        Format::Tiff => img.write_with_encoder(with_meta(TiffEncoder::new(file()?), meta))?,
        Format::Ico => write_ico(img, out)?,
        Format::Gif | Format::Bmp => {
            // Bu kodlayıcılar yalnızca 8 bit RGBA/RGB kabul eder.
            DynamicImage::ImageRgba8(img.to_rgba8())
                .save_with_format(out, target.image_format())?;
        }
    }
    Ok(())
}

/// Şeffaf alanı verilen renge oturtur (JPG şeffaflık desteklemez).
pub(crate) fn flatten_on(img: &DynamicImage, [r, g, b]: [u8; 3]) -> DynamicImage {
    let rgba = img.to_rgba8();
    let mut bg = RgbaImage::from_pixel(rgba.width(), rgba.height(), Rgba([r, g, b, 255]));
    image::imageops::overlay(&mut bg, &rgba, 0, 0);
    DynamicImage::ImageRgba8(bg).to_rgb8().into()
}

/// Görüntüyü `max_w`×`max_h` kutusuna sığdırır; 0 "sınırsız" demektir. Büyütme yapmaz.
fn shrink_to_fit(img: DynamicImage, max_w: u32, max_h: u32) -> DynamicImage {
    let mw = if max_w == 0 { u32::MAX } else { max_w };
    let mh = if max_h == 0 { u32::MAX } else { max_h };
    if img.width() <= mw && img.height() <= mh {
        return img;
    }
    img.resize(mw, mh, FilterType::Lanczos3)
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
                format!("kare.{}", target.ext())
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

    #[test]
    fn resize_keeps_aspect_and_never_upscales() {
        let d = temp_dir("resize");
        let input = sample_png(&d); // 64x48
        let small = Options {
            max_size: Some((32, 32)),
            suffix: Some("kucuk".into()),
            ..Options::default()
        };
        let out = convert_file_with(&input, Format::Png, &small).unwrap();
        assert_eq!(out.file_name().unwrap().to_string_lossy(), "kare-kucuk.png");
        let img = image::open(&out).unwrap();
        assert_eq!((img.width(), img.height()), (32, 24));

        let huge = Options {
            max_size: Some((1000, 0)),
            ..Options::default()
        };
        let out = convert_file_with(&input, Format::Jpg, &huge).unwrap();
        let img = image::open(out).unwrap();
        assert_eq!((img.width(), img.height()), (64, 48));
        let _ = fs::remove_dir_all(&d);
    }

    /// Make = "Abc", Orientation = 6 (90° saat yönünde) içeren küçük bir EXIF (TIFF) bloğu.
    fn sample_exif() -> Vec<u8> {
        let mut e = b"II*\0\x08\0\0\0".to_vec();
        e.extend_from_slice(&2u16.to_le_bytes());
        // Make: ASCII, 4 bayt, değer satır içinde.
        e.extend_from_slice(&[0x0f, 0x01, 2, 0, 4, 0, 0, 0, b'A', b'b', b'c', 0]);
        // Orientation: SHORT, 1 adet, 6.
        e.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
        e.extend_from_slice(&0u32.to_le_bytes());
        e
    }

    fn read_meta(path: &Path) -> (u32, u32, Meta, Orientation) {
        let mut dec = ImageReader::open(path)
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        let (w, h) = dec.dimensions();
        let icc = dec.icc_profile().unwrap();
        let exif = dec.exif_metadata().unwrap();
        let o = dec.orientation().unwrap();
        (w, h, Meta { icc, exif }, o)
    }

    #[test]
    fn exif_and_icc_survive_and_orientation_is_applied_once() {
        let d = temp_dir("meta");
        let input = d.join("kamera.jpg");
        let icc: Vec<u8> = (0..200u8).collect();
        {
            let px = RgbaImage::from_pixel(40, 20, Rgba([10, 200, 30, 255]));
            let rgb = DynamicImage::ImageRgba8(px).to_rgb8();
            let mut enc = JpegEncoder::new(BufWriter::new(File::create(&input).unwrap()));
            enc.set_icc_profile(icc.clone()).unwrap();
            enc.set_exif_metadata(sample_exif()).unwrap();
            DynamicImage::ImageRgb8(rgb)
                .write_with_encoder(enc)
                .unwrap();
        }
        let (_, _, _, o) = read_meta(&input);
        assert_eq!(o, Orientation::Rotate90);

        for target in [Format::Jpg, Format::Png, Format::Webp] {
            let o = Options {
                suffix: Some(format!("{target:?}")),
                ..Options::default()
            };
            let out = convert_file_with(&input, target, &o).unwrap();
            let (w, h, meta, orient) = read_meta(&out);
            assert_eq!((w, h), (20, 40), "{target:?}: yön pikselere uygulanmalı");
            assert_eq!(
                orient,
                Orientation::NoTransforms,
                "{target:?}: yön iki kez uygulanmamalı"
            );
            assert_eq!(
                meta.icc.as_deref(),
                Some(&icc[..]),
                "{target:?}: ICC korunmalı"
            );
            let exif = meta
                .exif
                .unwrap_or_else(|| panic!("{target:?}: EXIF korunmalı"));
            assert!(
                exif.windows(3).any(|w| w == b"Abc"),
                "{target:?}: Make kaybolmuş"
            );
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn lower_jpeg_quality_gives_smaller_file_and_background_applies() {
        let d = temp_dir("quality");
        let mut noisy = RgbaImage::new(128, 128);
        for (x, y, p) in noisy.enumerate_pixels_mut() {
            *p = Rgba([(x * 7 + y * 13) as u8, (x * 3) as u8, (y * 5) as u8, 255]);
        }
        let input = d.join("gurultu.png");
        noisy.save(&input).unwrap();
        let size = |q| {
            let o = Options {
                jpeg_quality: q,
                suffix: Some(format!("q{q}")),
                ..Options::default()
            };
            fs::metadata(convert_file_with(&input, Format::Jpg, &o).unwrap())
                .unwrap()
                .len()
        };
        assert!(size(20) < size(95));

        let clear = d.join("seffaf.png");
        RgbaImage::new(32, 32).save(&clear).unwrap();
        let black = Options {
            background: [0, 0, 0],
            ..Options::default()
        };
        let px = image::open(convert_file_with(&clear, Format::Jpg, &black).unwrap())
            .unwrap()
            .to_rgb8()
            .get_pixel(16, 16)
            .0;
        assert!(px.iter().all(|&c| c < 15), "arka plan siyah olmalı: {px:?}");
        let _ = fs::remove_dir_all(&d);
    }
}
