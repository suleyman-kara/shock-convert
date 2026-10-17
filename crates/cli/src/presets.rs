//! Preset'ler: sağ tık menüsündeki her girdinin tanımı. Yerleşik varsayılanlara,
//! `presets.toml` dosyasındaki kullanıcı preset'leri eklenir (aynı `id` varsayılanı ezer).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shock_convert_core::Format;

use crate::config;

/// Tek desteklenen preset türü. Eski sürümlerin `ffmpeg`, `pdf` ve `pdf-tool` preset'leri
/// okunur ama uyarıyla atlanır.
const IMAGE_KIND: &str = "image";

#[derive(Debug, Clone)]
pub struct Preset {
    pub id: String,
    pub label: String,
    /// Çıktı uzantısı (`jpg`, `webp`, ...).
    pub output: String,
    /// Menünün görüneceği dosya uzantıları (gruplar açılmış, küçük harf).
    pub input: Vec<String>,
    pub suffix: Option<String>,
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub background: Option<[u8; 3]>,
}

#[derive(Debug)]
pub struct Settings {
    pub presets: Vec<Preset>,
    /// Ayarlar ve özel dönüştürme pencereleri için Edge/Chrome yolu (yoksa otomatik bulunur).
    pub browser_path: Option<PathBuf>,
    /// Çıktı kaynağın tarihlerini taşısın mı (varsayılan evet).
    pub keep_dates: bool,
    /// EXIF politikası (varsayılan: koru).
    pub metadata: shock_convert_core::Metadata,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            presets: Vec::new(),
            browser_path: None,
            keep_dates: true,
            metadata: shock_convert_core::Metadata::default(),
        }
    }
}

/// `presets.toml`'un ham hali (ayarlar arayüzü de bunu okur ve yazar). Eski sürümlerin
/// anahtarları (`ffmpeg_path`, `pdf_engine` ...) yok sayılır; arayüz kaydederken dosyada kalırlar.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct FileModel {
    pub browser_path: Option<String>,
    pub keep_dates: Option<bool>,
    pub metadata: Option<String>,
    #[serde(default)]
    pub hide: Vec<String>,
    #[serde(default)]
    pub preset: Vec<RawPreset>,
}

fn image_kind() -> String {
    IMAGE_KIND.into()
}

/// Dosyadaki bir preset, doğrulanmamış haliyle.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RawPreset {
    pub id: String,
    pub label: String,
    #[serde(default = "image_kind")]
    pub kind: String,
    pub output: String,
    pub input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

/// Dosyayı ham haliyle okur. Dosya yoksa boş model döner.
pub(crate) fn read_file() -> Result<FileModel, String> {
    let path = config::presets_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FileModel::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// `image` grubunu uzantılara açar; diğer değerler uzantı sayılır.
fn expand_inputs(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim().trim_start_matches('.').to_ascii_lowercase();
        let group: Vec<String> = match item.as_str() {
            "image" => Format::ALL
                .iter()
                .flat_map(|f| f.input_exts().iter().map(|e| e.to_string()))
                .collect(),
            _ => vec![item],
        };
        for e in group {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

impl RawPreset {
    pub(crate) fn into_preset(self) -> Result<Preset, String> {
        if self.id.is_empty() || self.id.contains([':', '\\', '/', ' ']) {
            return Err(format!("invalid id: `{}`", self.id));
        }
        if self.kind != IMAGE_KIND {
            return Err(format!(
                "`{}`: kind \"{}\" is no longer supported (only \"image\")",
                self.id, self.kind
            ));
        }
        let output = self.output.trim_start_matches('.').to_ascii_lowercase();
        if Format::from_ext(&output).is_none() {
            return Err(format!("`{}`: `{output}` is not an image format", self.id));
        }
        let background = match &self.background {
            Some(c) => Some(parse_color(c).ok_or(format!("`{}`: invalid colour `{c}`", self.id))?),
            None => None,
        };
        let input = expand_inputs(&self.input);
        if input.is_empty() {
            return Err(format!("`{}`: input is empty", self.id));
        }
        Ok(Preset {
            id: self.id,
            label: self.label,
            output,
            input,
            suffix: self.suffix,
            quality: self.quality,
            max_width: self.max_width,
            max_height: self.max_height,
            background,
        })
    }
}

/// Yerleşik preset'ler: her görüntü formatı için bir girdi. Menü sırası bu sıradır.
pub fn defaults() -> Vec<Preset> {
    Format::ALL
        .iter()
        .map(|f| Preset {
            id: f.ext().into(),
            label: f.label().into(),
            output: f.ext().into(),
            input: expand_inputs(&["image".into()]),
            suffix: None,
            quality: None,
            max_width: None,
            max_height: None,
            background: None,
        })
        .collect()
}

/// Varsayılanlar + `presets.toml`. Bozuk girdiler atlanır, nedenleri uyarı olarak döner.
pub fn load() -> (Settings, Vec<String>) {
    let mut warnings = Vec::new();
    let mut settings = Settings {
        presets: defaults(),
        ..Settings::default()
    };

    let path = config::presets_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        match toml::from_str::<FileModel>(&text) {
            Ok(file) => {
                settings.browser_path = file
                    .browser_path
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from);
                settings.keep_dates = file.keep_dates.unwrap_or(true);
                if let Some(m) = file.metadata.as_deref() {
                    match shock_convert_core::Metadata::parse(m) {
                        Some(v) => settings.metadata = v,
                        None => warnings.push(format!(
                            "{}: invalid metadata \"{m}\" (keep | strip-location | strip)",
                            path.display()
                        )),
                    }
                }
                for raw in file.preset {
                    match raw.into_preset() {
                        Ok(p) => match settings.presets.iter_mut().find(|e| e.id == p.id) {
                            Some(slot) => *slot = p,
                            None => settings.presets.push(p),
                        },
                        Err(e) => warnings.push(format!("{}: {e}", path.display())),
                    }
                }
                settings.presets.retain(|p| !file.hide.contains(&p.id));
            }
            Err(e) => warnings.push(format!("{}: {e}", path.display())),
        }
    }
    (settings, warnings)
}

const SAMPLE: &str = r##"# Shock Convert presets.
# After editing, refresh the menu with:  shock-convert register
#
# Built-in presets (PNG, JPG, WebP, BMP, GIF, TIFF, ICO) are always there. The presets below are
# added to them; a preset with the same `id` replaces the built-in one.

# Should the output keep the source file's dates (created/modified):
# keep_dates = true
# Photo information (EXIF: camera, date, location): "keep", "strip-location" or "strip"
# metadata = "keep"

# Built-in presets to hide from the menu:
# hide = ["bmp", "tiff", "ico"]

# Edge or Chrome is found automatically for the settings window; give a path if needed:
# browser_path = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"

# Example: a smaller JPG for the web  ->  photo-web.jpg
# [[preset]]
# id = "web-jpg"
# label = "JPG (web, 1920px)"
# output = "jpg"
# input = ["image"]         # the "image" group or individual extensions
# suffix = "web"
# quality = 80              # JPG quality (1-100)
# max_width = 1920          # aspect ratio is kept, never upscaled
# max_height = 1920
# background = "#ffffff"    # colour for transparent areas in JPG
"##;

/// Açıklamalı örnek dosyanın metni (dosya yokken ayarlar arayüzü bunun üzerine yazar).
pub(crate) fn sample() -> &'static str {
    SAMPLE
}

/// `presets.toml` yoksa açıklamalı örnek dosyayı yazar (varsa dokunmaz). Yolu döndürür.
pub fn ensure_sample() -> std::io::Result<PathBuf> {
    let path = config::presets_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, SAMPLE)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Ortam değişkeni süreç geneli olduğu için testler birbirini beklesin.
    static ENV: Mutex<()> = Mutex::new(());

    fn with_config(toml: Option<&str>, f: impl FnOnce()) {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("shock-convert-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(t) = toml {
            std::fs::write(dir.join("presets.toml"), t).unwrap();
        }
        unsafe { std::env::set_var("SHOCK_CONVERT_CONFIG_DIR", &dir) };
        f();
        unsafe { std::env::remove_var("SHOCK_CONVERT_CONFIG_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn defaults_cover_every_image_format_once() {
        let d = defaults();
        let mut ids: Vec<_> = d.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), d.len());
        assert_eq!(d.len(), Format::ALL.len());
        assert!(d.iter().any(|p| p.id == "jpg" && p.input.contains(&"png".into())));
    }

    #[test]
    fn user_presets_extend_override_and_hide() {
        with_config(
            Some(
                r##"
hide = ["bmp"]
[[preset]]
id = "web-jpg"
label = "Web"
kind = "image"
output = "jpg"
input = ["image"]
quality = 70
max_width = 1920
background = "#000000"
[[preset]]
id = "png"
label = "PNG (only from WebP)"
output = "png"
input = ["webp"]
[[preset]]
id = "bad id"
label = "x"
output = "png"
input = ["image"]
"##,
            ),
            || {
                let (s, warnings) = load();
                assert_eq!(warnings.len(), 1, "{warnings:?}");
                assert!(s.presets.iter().all(|p| p.id != "bmp"));
                let web = s.presets.iter().find(|p| p.id == "web-jpg").unwrap();
                assert_eq!(
                    (web.quality, web.max_width, web.background),
                    (Some(70), Some(1920), Some([0, 0, 0]))
                );
                let png = s.presets.iter().find(|p| p.id == "png").unwrap();
                assert_eq!(png.input, ["webp"]);
                assert_eq!(s.presets.iter().filter(|p| p.id == "png").count(), 1);
            },
        );
    }

    #[test]
    fn old_media_and_pdf_presets_and_keys_are_skipped_with_a_warning() {
        with_config(
            Some(
                r##"
ffmpeg_path = "C:\\ffmpeg.exe"
pdf_engine = "office"
keep_dates = false
[[preset]]
id = "mp4-720p"
label = "MP4 (720p)"
kind = "ffmpeg"
output = "mp4"
input = ["video"]
args = ["-vf", "scale=-2:720"]
[[preset]]
id = "pdf-300"
label = "PDF"
kind = "pdf-tool"
tool = "to-images"
output = "jpg"
input = ["pdf"]
dpi = 300
"##,
            ),
            || {
                let (s, warnings) = load();
                assert_eq!(warnings.len(), 2, "{warnings:?}");
                assert!(warnings.iter().all(|w| w.contains("no longer supported")));
                assert!(!s.keep_dates, "the rest of the file still applies");
                assert_eq!(s.presets.len(), defaults().len());
            },
        );
    }

    #[test]
    fn behavior_settings_default_on_and_can_be_changed() {
        with_config(None, || {
            let (s, _) = load();
            assert!(s.keep_dates);
            assert_eq!(s.metadata, shock_convert_core::Metadata::Keep);
        });
        with_config(
            Some("keep_dates = false\nmetadata = \"strip-location\"\n"),
            || {
                let (s, w) = load();
                assert!(w.is_empty(), "{w:?}");
                assert!(!s.keep_dates);
                assert_eq!(s.metadata, shock_convert_core::Metadata::StripLocation);
            },
        );
        with_config(Some("metadata = \"sil\"\n"), || {
            let (_, w) = load();
            assert!(w.iter().any(|w| w.contains("metadata")), "{w:?}");
        });
    }

    #[test]
    fn broken_file_keeps_defaults_and_warns() {
        with_config(Some("preset = ["), || {
            let (s, warnings) = load();
            assert_eq!(s.presets.len(), defaults().len());
            assert_eq!(warnings.len(), 1);
        });
    }

    #[test]
    fn sample_file_is_valid_and_only_written_once() {
        with_config(None, || {
            let p = ensure_sample().unwrap();
            let first = std::fs::read_to_string(&p).unwrap();
            std::fs::write(&p, "# mine\n").unwrap();
            ensure_sample().unwrap();
            assert_eq!(std::fs::read_to_string(&p).unwrap(), "# mine\n");
            assert!(toml::from_str::<FileModel>(&first).is_ok());
            let (s, w) = load();
            assert!(w.is_empty() && s.presets.len() == defaults().len());
        });
    }
}
