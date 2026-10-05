use image::ImageFormat;

/// Dönüştürme hedefi olabilen görüntü formatları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpg,
    Webp,
    Bmp,
    Gif,
    Tiff,
    Ico,
}

impl Format {
    pub const ALL: [Format; 7] = [
        Format::Png,
        Format::Jpg,
        Format::Webp,
        Format::Bmp,
        Format::Gif,
        Format::Tiff,
        Format::Ico,
    ];

    /// Çıktı dosyasının uzantısı ve dosya adına eklenen ek (`foto-jpg.jpg`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg => "jpg",
            Format::Webp => "webp",
            Format::Bmp => "bmp",
            Format::Gif => "gif",
            Format::Tiff => "tiff",
            Format::Ico => "ico",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpg => "JPG",
            Format::Webp => "WebP",
            Format::Bmp => "BMP",
            Format::Gif => "GIF",
            Format::Tiff => "TIFF",
            Format::Ico => "ICO",
        }
    }

    /// Bu formatın dosya sistemindeki tüm bilinen uzantıları.
    pub fn input_exts(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png"],
            Format::Jpg => &["jpg", "jpeg", "jfif"],
            Format::Webp => &["webp"],
            Format::Bmp => &["bmp"],
            Format::Gif => &["gif"],
            Format::Tiff => &["tif", "tiff"],
            Format::Ico => &["ico"],
        }
    }

    /// Büyük/küçük harfe duyarsız uzantı çözümlemesi (`"JPEG"` -> `Jpg`).
    pub fn from_ext(ext: &str) -> Option<Format> {
        let ext = ext.to_ascii_lowercase();
        Format::ALL
            .into_iter()
            .find(|f| f.input_exts().contains(&ext.as_str()))
    }

    /// Komut satırındaki kimlik (`png`, `jpg`, ...) için de aynı çözümleme.
    pub fn from_id(id: &str) -> Option<Format> {
        Format::from_ext(id)
    }

    pub(crate) fn image_format(self) -> ImageFormat {
        match self {
            Format::Png => ImageFormat::Png,
            Format::Jpg => ImageFormat::Jpeg,
            Format::Webp => ImageFormat::WebP,
            Format::Bmp => ImageFormat::Bmp,
            Format::Gif => ImageFormat::Gif,
            Format::Tiff => ImageFormat::Tiff,
            Format::Ico => ImageFormat::Ico,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aliases_case_insensitively() {
        assert_eq!(Format::from_ext("JPEG"), Some(Format::Jpg));
        assert_eq!(Format::from_ext("tif"), Some(Format::Tiff));
        assert_eq!(Format::from_ext("txt"), None);
    }
}
