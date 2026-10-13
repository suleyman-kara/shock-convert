//! OOXML (docx/xlsx/pptx) paketi: zip içinden parça okuma, ilişki (rels) çözümleme, resim gömme.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use zip::ZipArchive;

use super::xml::{self, Ev};

/// Zip bombası ve devasa parçalara karşı tek parça okuma üst sınırı.
const MAX_PART_BYTES: u64 = 256 * 1024 * 1024;

pub struct Package {
    zip: ZipArchive<File>,
}

#[derive(Debug, Clone)]
pub struct Rel {
    pub target: String,
    pub external: bool,
}

impl Package {
    pub fn open(path: &Path) -> Result<Package, String> {
        let file = File::open(path).map_err(|e| format!("dosya açılamadı: {e}"))?;
        let zip =
            ZipArchive::new(file).map_err(|e| format!("geçerli bir Office dosyası değil: {e}"))?;
        Ok(Package { zip })
    }

    pub fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        let mut entry = self.zip.by_name(name).ok()?;
        if entry.size() > MAX_PART_BYTES {
            return None;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .by_ref()
            .take(MAX_PART_BYTES)
            .read_to_end(&mut buf)
            .ok()?;
        Some(buf)
    }

    pub fn read_string(&mut self, name: &str) -> Option<String> {
        let bytes = self.read(name)?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// `base`'in (ör. `word/document.xml`) yanındaki `_rels/<ad>.rels` ilişkileri.
    pub fn rels_of(&mut self, base: &str) -> HashMap<String, Rel> {
        let (dir, file) = base.rsplit_once('/').unwrap_or(("", base));
        let rels_path = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        let mut map = HashMap::new();
        let Some(text) = self.read_string(&rels_path) else {
            return map;
        };
        let _ = xml::parse(&text, |ev| {
            if let Ev::Start(t) = ev
                && t.name == "Relationship"
                && let (Some(id), Some(target)) = (t.get("Id"), t.get("Target"))
            {
                map.insert(
                    id.to_string(),
                    Rel {
                        target: target.to_string(),
                        external: t.get("TargetMode") == Some("External"),
                    },
                );
            }
            true
        });
        map
    }

    /// İlişki hedefindeki resmi `data:` URI olarak döndürür (desteklenmeyen türlerde `None`).
    pub fn image_data_uri(&mut self, base_dir: &str, rel: &Rel) -> Option<String> {
        if rel.external {
            return None;
        }
        let path = resolve(base_dir, &rel.target);
        let mime = match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            _ => return None, // emf/wmf/tiff vb. tarayıcıda görüntülenemez
        };
        let bytes = self.read(&path)?;
        Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
    }
}

/// `base_dir` (ör. `word`) altındaki göreli hedefi paket köküne göre çözer; `..` ve `/` desteklenir.
pub fn resolve(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base_dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn resolves_relative_and_absolute_targets() {
        assert_eq!(resolve("word", "media/image1.png"), "word/media/image1.png");
        assert_eq!(resolve("ppt/slides", "../media/a.png"), "ppt/media/a.png");
        assert_eq!(resolve("word", "/word/media/x.png"), "word/media/x.png");
    }
}
