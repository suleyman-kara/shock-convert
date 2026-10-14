//! Preset'ler: sağ tık menüsündeki her girdinin tanımı. Yerleşik varsayılanlara,
//! `presets.toml` dosyasındaki kullanıcı preset'leri eklenir (aynı `id` varsayılanı ezer).

use std::path::PathBuf;

use serde::Deserialize;
use shock_convert_core::Format;

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Süreç içinde, harici program olmadan (Rust `image`).
    Image,
    /// `ffmpeg` ile (ses/video).
    Ffmpeg,
    /// PDF'e (görüntü, metin, Office belgeleri; bkz. `pdf` modülü).
    Pdf,
    /// PDF işlemi: birleştir, böl, döndür (`tool` alanı; bkz. `pdf::tools`).
    #[serde(rename = "pdf-tool")]
    PdfTool,
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    /// Çıktı uzantısı (`jpg`, `mp3`, ...).
    pub output: String,
    /// Menünün görüneceği dosya uzantıları (gruplar açılmış, küçük harf).
    pub input: Vec<String>,
    pub suffix: Option<String>,
    // Görüntü ayarları
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub background: Option<[u8; 3]>,
    // ffmpeg
    pub args: Vec<String>,
    // pdf-tool: merge | split | rotate-right | rotate-left | rotate-180 | to-images | to-text | compress
    pub tool: Option<crate::pdf::tools::Tool>,
    /// pdf-tool to-images: çözünürlük (varsayılan 150).
    pub dpi: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Settings {
    pub presets: Vec<Preset>,
    pub ffmpeg_path: Option<PathBuf>,
    pub browser_path: Option<PathBuf>,
    pub libreoffice_path: Option<PathBuf>,
    pub pdf_engine: crate::pdf::Engine,
}

#[derive(Debug, Default, Deserialize)]
struct FileModel {
    ffmpeg_path: Option<String>,
    browser_path: Option<String>,
    libreoffice_path: Option<String>,
    pdf_engine: Option<String>,
    #[serde(default)]
    hide: Vec<String>,
    #[serde(default)]
    preset: Vec<RawPreset>,
}

#[derive(Debug, Deserialize)]
struct RawPreset {
    id: String,
    label: String,
    kind: Kind,
    output: String,
    input: Vec<String>,
    suffix: Option<String>,
    quality: Option<u8>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    background: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    tool: Option<String>,
    dpi: Option<u32>,
}

const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac", "wma", "aiff", "aif",
];
const DOCUMENT: &[&str] = &[
    "docx", "xlsx", "pptx", "doc", "xls", "ppt", "rtf", "odt", "ods", "odp", "txt", "md",
    "markdown", "html", "htm",
];
const VIDEO: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "wmv", "flv", "m4v", "3gp", "mpg", "mpeg", "ts",
];

/// `image` / `audio` / `video` / `media` gruplarını uzantılara açar; diğer değerler uzantı sayılır.
fn expand_inputs(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim().trim_start_matches('.').to_ascii_lowercase();
        let group: Vec<String> = match item.as_str() {
            "image" => Format::ALL
                .iter()
                .flat_map(|f| f.input_exts().iter().map(|e| e.to_string()))
                .collect(),
            "audio" => AUDIO.iter().map(|e| e.to_string()).collect(),
            "video" => VIDEO.iter().map(|e| e.to_string()).collect(),
            "media" => AUDIO.iter().chain(VIDEO).map(|e| e.to_string()).collect(),
            "document" => DOCUMENT.iter().map(|e| e.to_string()).collect(),
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
    fn into_preset(self) -> Result<Preset, String> {
        if self.id.is_empty() || self.id.contains([':', '\\', '/', ' ']) {
            return Err(format!("geçersiz id: `{}`", self.id));
        }
        let output = self.output.trim_start_matches('.').to_ascii_lowercase();
        if output.is_empty() || !output.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("`{}`: geçersiz output", self.id));
        }
        if self.kind == Kind::Image && Format::from_ext(&output).is_none() {
            return Err(format!("`{}`: `{output}` görüntü formatı değil", self.id));
        }
        if self.kind == Kind::Pdf && output != "pdf" {
            return Err(format!(
                "`{}`: kind = \"pdf\" için output \"pdf\" olmalı",
                self.id
            ));
        }
        let tool = match (self.kind, self.tool.as_deref()) {
            (Kind::PdfTool, Some(t)) => Some(crate::pdf::tools::Tool::parse(t).ok_or(format!(
                "`{}`: geçersiz tool `{t}` (merge | split | rotate-right | rotate-left | rotate-180 | to-images | to-text | compress)",
                self.id
            ))?),
            (Kind::PdfTool, None) => return Err(format!("`{}`: kind = \"pdf-tool\" için tool gerekli", self.id)),
            _ => None,
        };
        if let Some(tool) = tool {
            use crate::pdf::tools::Tool;
            let (ok, expected) = match tool {
                Tool::ToImages => (
                    matches!(output.as_str(), "png" | "jpg"),
                    "\"png\" ya da \"jpg\"",
                ),
                Tool::ToText => (output == "txt", "\"txt\""),
                _ => (output == "pdf", "\"pdf\""),
            };
            if !ok {
                return Err(format!(
                    "`{}`: bu PDF işlemi için output {expected} olmalı",
                    self.id
                ));
            }
        }
        let background = match &self.background {
            Some(c) => Some(parse_color(c).ok_or(format!("`{}`: geçersiz renk `{c}`", self.id))?),
            None => None,
        };
        let input = expand_inputs(&self.input);
        if input.is_empty() {
            return Err(format!("`{}`: input boş", self.id));
        }
        Ok(Preset {
            id: self.id,
            label: self.label,
            kind: self.kind,
            output,
            input,
            suffix: self.suffix,
            quality: self.quality,
            max_width: self.max_width,
            max_height: self.max_height,
            background,
            args: self.args,
            tool,
            dpi: self.dpi,
        })
    }
}

fn image_preset(f: Format) -> Preset {
    Preset {
        id: f.ext().into(),
        label: f.label().into(),
        kind: Kind::Image,
        output: f.ext().into(),
        input: expand_inputs(&["image".into()]),
        suffix: None,
        quality: None,
        max_width: None,
        max_height: None,
        background: None,
        args: vec![],
        tool: None,
        dpi: None,
    }
}

fn ffmpeg_preset(id: &str, label: &str, input: &[&str], args: &[&str]) -> Preset {
    Preset {
        id: id.into(),
        label: label.into(),
        kind: Kind::Ffmpeg,
        output: id.into(),
        input: expand_inputs(&input.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
        suffix: None,
        quality: None,
        max_width: None,
        max_height: None,
        background: None,
        args: args.iter().map(|s| s.to_string()).collect(),
        tool: None,
        dpi: None,
    }
}

/// Yerleşik preset'ler. Menü sırası bu sıradır.
pub fn defaults() -> Vec<Preset> {
    let mut v: Vec<Preset> = Format::ALL.iter().map(|&f| image_preset(f)).collect();
    v.push(Preset {
        id: "pdf".into(),
        label: "PDF".into(),
        kind: Kind::Pdf,
        output: "pdf".into(),
        input: expand_inputs(&["image".into(), "document".into()]),
        suffix: None,
        quality: None,
        max_width: None,
        max_height: None,
        background: None,
        args: vec![],
        tool: None,
        dpi: None,
    });
    use crate::pdf::tools::Tool;
    let pdf = || vec!["pdf".to_string()];
    for (id, label, tool, output, suffix, input) in [
        (
            "pdf-merge",
            "PDF: Birleştir",
            Tool::Merge,
            "pdf",
            "birleşik",
            vec!["pdf".to_string(), "image".to_string()],
        ),
        (
            "pdf-split",
            "PDF: Sayfalara böl",
            Tool::Split,
            "pdf",
            "sayfa",
            pdf(),
        ),
        (
            "pdf-rotate-right",
            "PDF: 90° sağa döndür",
            Tool::RotateRight,
            "pdf",
            "döndürülmüş",
            pdf(),
        ),
        (
            "pdf-rotate-left",
            "PDF: 90° sola döndür",
            Tool::RotateLeft,
            "pdf",
            "döndürülmüş",
            pdf(),
        ),
        (
            "pdf-rotate-180",
            "PDF: 180° döndür",
            Tool::Rotate180,
            "pdf",
            "döndürülmüş",
            pdf(),
        ),
        (
            "pdf-compress",
            "PDF: Küçült",
            Tool::Compress,
            "pdf",
            "küçük",
            pdf(),
        ),
        (
            "pdf-png",
            "PDF: Görüntülere çevir (PNG)",
            Tool::ToImages,
            "png",
            "sayfa",
            pdf(),
        ),
        (
            "pdf-jpg",
            "PDF: Görüntülere çevir (JPG)",
            Tool::ToImages,
            "jpg",
            "sayfa",
            pdf(),
        ),
        (
            "pdf-txt",
            "PDF: Metne çevir (TXT)",
            Tool::ToText,
            "txt",
            "",
            pdf(),
        ),
    ] {
        v.push(Preset {
            id: id.into(),
            label: label.into(),
            kind: Kind::PdfTool,
            output: output.into(),
            input: expand_inputs(&input),
            suffix: (!suffix.is_empty()).then(|| suffix.into()),
            quality: None,
            max_width: None,
            max_height: None,
            background: None,
            args: vec![],
            tool: Some(tool),
            dpi: None,
        });
    }
    v.extend([
        ffmpeg_preset(
            "mp3",
            "MP3",
            &["media"],
            &[
                "-vn",
                "-c:a",
                "libmp3lame",
                "-q:a",
                "2",
                // Windows Gezgini ID3v2.4 etiketlerini eksik gösterir; v2.3 + v1 en uyumlusu.
                "-id3v2_version",
                "3",
                "-write_id3v1",
                "1",
            ],
        ),
        ffmpeg_preset("wav", "WAV", &["media"], &["-vn", "-c:a", "pcm_s16le"]),
        ffmpeg_preset("flac", "FLAC", &["media"], &["-vn", "-c:a", "flac"]),
        ffmpeg_preset(
            "ogg",
            "OGG",
            &["media"],
            &["-vn", "-c:a", "libvorbis", "-q:a", "5"],
        ),
        ffmpeg_preset(
            "m4a",
            "M4A (AAC)",
            &["media"],
            &["-vn", "-c:a", "aac", "-b:a", "192k"],
        ),
        ffmpeg_preset(
            "mp4",
            "MP4 (H.264)",
            &["video"],
            &[
                "-c:v",
                "libx264",
                "-crf",
                "23",
                "-preset",
                "medium",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
            ],
        ),
        ffmpeg_preset(
            "webm",
            "WebM (VP9)",
            &["video"],
            &[
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "32",
                "-b:v",
                "0",
                "-c:a",
                "libopus",
            ],
        ),
        ffmpeg_preset(
            "gif",
            "GIF (animasyon)",
            &["video"],
            &[
                "-vf", // Tek geçişte palet üretip kullanır: 256 renge düşerken bantlaşma ve renk kayması azalır.
                "fps=12,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                "-an",
            ],
        ),
    ]);
    // `gif` kimliği görüntü GIF'iyle çakışmasın.
    if let Some(p) = v.iter_mut().rev().find(|p| p.id == "gif") {
        p.id = "video-gif".into();
    }
    v
}

/// Varsayılanlar + `presets.toml`. Bozuk girdiler atlanır, nedenleri uyarı olarak döner.
pub fn load() -> (Settings, Vec<String>) {
    let mut warnings = Vec::new();
    let mut presets = defaults();
    let mut ffmpeg_path = None;
    let (mut browser_path, mut libreoffice_path) = (None, None);
    let mut pdf_engine = crate::pdf::Engine::default();

    let path = config::presets_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        match toml::from_str::<FileModel>(&text) {
            Ok(file) => {
                let some_path = |p: Option<String>| p.filter(|p| !p.is_empty()).map(PathBuf::from);
                ffmpeg_path = some_path(file.ffmpeg_path);
                browser_path = some_path(file.browser_path);
                libreoffice_path = some_path(file.libreoffice_path);
                if let Some(e) = file.pdf_engine.as_deref() {
                    match crate::pdf::Engine::parse(e) {
                        Some(engine) => pdf_engine = engine,
                        None => warnings.push(format!(
                            "{}: pdf_engine \"{e}\" geçersiz (auto | office | libreoffice | builtin)",
                            path.display()
                        )),
                    }
                }
                for raw in file.preset {
                    match raw.into_preset() {
                        Ok(p) => match presets.iter_mut().find(|e| e.id == p.id) {
                            Some(slot) => *slot = p,
                            None => presets.push(p),
                        },
                        Err(e) => warnings.push(format!("{}: {e}", path.display())),
                    }
                }
                presets.retain(|p| !file.hide.contains(&p.id));
            }
            Err(e) => warnings.push(format!("{}: {e}", path.display())),
        }
    }
    (
        Settings {
            presets,
            ffmpeg_path,
            browser_path,
            libreoffice_path,
            pdf_engine,
        },
        warnings,
    )
}

const SAMPLE: &str = r##"# Shock Convert preset'leri.
# Düzenledikten sonra menüyü yenilemek için:  shock-convert register
#
# Yerleşik preset'ler (PNG, JPG, MP3, MP4 ...) her zaman vardır. Aşağıdakiler onlara eklenir;
# aynı `id` ile yeni bir preset yerleşik olanı ezer.

# ffmpeg otomatik bulunur (kurulum klasöründeki ffmpeg\ffmpeg.exe, sonra PATH).
# Başka bir yerdeyse yolunu verin:
# ffmpeg_path = "C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe"

# PDF: Office belgeleri (DOCX/XLSX/PPTX, DOC/XLS/PPT, RTF, ODT/ODS/ODP) için motor.
#   auto        = sırayla Microsoft Office, LibreOffice, yerleşik dönüştürücü; olmayan ya da
#                 başarısız olan atlanır (varsayılan)
#   office      = yalnızca Microsoft Office (Word/Excel/PowerPoint'in kendi PDF çıktısı)
#   libreoffice = yalnızca LibreOffice
#   builtin     = her zaman yerleşik dönüştürücü (hızlı, orta doğruluk)
# pdf_engine = "auto"
# Edge/Chrome ve LibreOffice otomatik bulunur; gerekirse yollarını verin:
# browser_path = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"
# libreoffice_path = "C:\\Program Files\\LibreOffice\\program\\soffice.exe"

# PDF işlemleri (kind = "pdf-tool"), tool:
#   merge | split | rotate-right | rotate-left | rotate-180   -> output = "pdf"
#   compress  (quality = JPG kalitesi, max_width = resimlerin en uzun kenarı; varsayılan 70 / 1600)
#   to-images (output = "png" | "jpg", dpi = çözünürlük; varsayılan 150)
#   to-text   (output = "txt")

# Menüden gizlemek istediğiniz yerleşik preset'ler:
# hide = ["bmp", "tiff", "ico"]

# Örnek: web için küçültülmüş JPG  ->  foto-web.jpg
# [[preset]]
# id = "web-jpg"
# label = "JPG (web, 1920px)"
# kind = "image"            # image | ffmpeg | pdf
# output = "jpg"
# input = ["image"]         # gruplar: image, audio, video, media, document; ya da tek tek uzantılar
# suffix = "web"
# quality = 80              # JPG kalitesi (1-100)
# max_width = 1920          # oran korunur, asla büyütülmez
# max_height = 1920
# background = "#ffffff"    # şeffaf alanın JPG'deki rengi

# Örnek: videoyu 720p MP4'e küçült  ->  film-720p.mp4
# [[preset]]
# id = "mp4-720p"
# label = "MP4 (720p)"
# kind = "ffmpeg"
# output = "mp4"
# input = ["video"]
# suffix = "720p"
# args = ["-vf", "scale=-2:720", "-c:v", "libx264", "-crf", "26", "-c:a", "aac"]
"##;

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
    fn defaults_have_unique_ids_and_known_kinds() {
        let d = defaults();
        let mut ids: Vec<_> = d.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), d.len());
        assert!(d.iter().any(|p| p.id == "jpg" && p.kind == Kind::Image));
        assert!(
            d.iter()
                .any(|p| p.id == "mp3" && p.input.contains(&"mp4".to_string()))
        );
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
id = "mp3"
label = "MP3 (yüksek)"
kind = "ffmpeg"
output = "mp3"
input = ["audio"]
args = ["-q:a", "0"]
[[preset]]
id = "bozuk id"
label = "x"
kind = "image"
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
                let mp3 = s.presets.iter().find(|p| p.id == "mp3").unwrap();
                assert_eq!(mp3.label, "MP3 (yüksek)");
                assert!(!mp3.input.contains(&"mp4".to_string()));
                assert_eq!(s.presets.iter().filter(|p| p.id == "mp3").count(), 1);
            },
        );
    }

    #[test]
    fn pdf_tool_presets_check_output_per_tool() {
        with_config(
            Some(
                r##"
[[preset]]
id = "pdf-tiff-300"
label = "PDF: 300 dpi JPG"
kind = "pdf-tool"
tool = "to-images"
output = "jpg"
input = ["pdf"]
dpi = 300
[[preset]]
id = "yanlis-metin"
label = "x"
kind = "pdf-tool"
tool = "to-text"
output = "pdf"
input = ["pdf"]
[[preset]]
id = "yanlis-goruntu"
label = "x"
kind = "pdf-tool"
tool = "to-images"
output = "gif"
input = ["pdf"]
"##,
            ),
            || {
                let (s, warnings) = load();
                assert_eq!(warnings.len(), 2, "{warnings:?}");
                assert!(
                    warnings
                        .iter()
                        .any(|w| w.contains("yanlis-metin") && w.contains("txt"))
                );
                let p = s.presets.iter().find(|p| p.id == "pdf-tiff-300").unwrap();
                assert_eq!((p.dpi, p.output.as_str()), (Some(300), "jpg"));
            },
        );
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
            std::fs::write(&p, "# benim\n").unwrap();
            ensure_sample().unwrap();
            assert_eq!(std::fs::read_to_string(&p).unwrap(), "# benim\n");
            assert!(toml::from_str::<FileModel>(&first).is_ok());
            let (s, w) = load();
            assert!(w.is_empty() && s.presets.len() == defaults().len());
        });
    }
}
