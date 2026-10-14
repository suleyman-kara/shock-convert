//! Veriye dayalı menü modeli: preset'ler (görüntü + ffmpeg) ve eklenti girdileri tek listede.

use std::path::PathBuf;

use shock_convert_core::{Format, Options};

use crate::plugins::{Plugin, PluginEntry};
use crate::presets::{Kind, Preset, Settings};

#[derive(Debug, Clone)]
pub struct FfmpegJob {
    pub ffmpeg_path: Option<PathBuf>,
    pub args: Vec<String>,
    pub suffix: Option<String>,
    pub output_ext: String,
}

#[derive(Debug, Clone)]
pub enum Action {
    Image { format: Format, options: Options },
    Ffmpeg(FfmpegJob),
    Pdf(crate::pdf::PdfJob),
    PdfTool(crate::pdf::tools::ToolJob),
    Plugin(PluginEntry),
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Komut satırındaki kimlik: `png`, `mp3`, `web-jpg` veya `<eklenti>:<girdi>`.
    pub id: String,
    pub label: String,
    /// Bu girdinin menüde görüneceği dosya uzantıları (küçük harf, noktasız).
    pub input_exts: Vec<String>,
    pub action: Action,
}

impl Entry {
    pub fn accepts(&self, ext: &str) -> bool {
        let ext = ext.to_ascii_lowercase();
        self.input_exts.contains(&ext)
    }
}

fn preset_entry(p: &Preset, s: &Settings) -> Option<Entry> {
    let own_exts: Vec<&str> = match Format::from_ext(&p.output) {
        Some(f) => f.input_exts().to_vec(),
        None => vec![p.output.as_str()],
    };
    let mut input_exts = p.input.clone();
    // Dosyayı kendi formatına çevirme girdisi gösterilmez. Ama küçültme yapan ya da kendi
    // `suffix`'ini tanımlayan preset bir "varyant"tır (örn. 720p MP4) ve kendi formatında da anlamlıdır.
    let resizes = p.max_width.is_some() || p.max_height.is_some();
    let is_variant = resizes || p.suffix.is_some() || p.kind == Kind::PdfTool;
    if !is_variant {
        input_exts.retain(|e| !own_exts.contains(&e.as_str()));
    }
    if input_exts.is_empty() {
        return None;
    }

    let action = match p.kind {
        Kind::Image => {
            let defaults = Options::default();
            Action::Image {
                format: Format::from_ext(&p.output)?,
                options: Options {
                    jpeg_quality: p.quality.unwrap_or(defaults.jpeg_quality),
                    max_size: resizes
                        .then(|| (p.max_width.unwrap_or(0), p.max_height.unwrap_or(0))),
                    background: p.background.unwrap_or(defaults.background),
                    suffix: p.suffix.clone(),
                },
            }
        }
        Kind::PdfTool => Action::PdfTool(crate::pdf::tools::ToolJob {
            tool: p.tool?,
            suffix: p.suffix.clone().unwrap_or_default(),
            image_format: Format::from_ext(&p.output).unwrap_or(Format::Png),
            dpi: p.dpi.unwrap_or(crate::pdf::tools::DEFAULT_DPI),
            quality: p.quality,
            max_side: p.max_width.max(p.max_height),
        }),
        Kind::Pdf => Action::Pdf(crate::pdf::PdfJob {
            browser_path: s.browser_path.clone(),
            libreoffice_path: s.libreoffice_path.clone(),
            engine: s.pdf_engine,
        }),
        Kind::Ffmpeg => Action::Ffmpeg(FfmpegJob {
            ffmpeg_path: s.ffmpeg_path.clone(),
            args: p.args.clone(),
            suffix: p.suffix.clone(),
            output_ext: p.output.clone(),
        }),
    };
    Some(Entry {
        id: p.id.clone(),
        label: p.label.clone(),
        input_exts,
        action,
    })
}

/// Menü sırası: preset'ler (varsayılanlar, sonra kullanıcınınkiler), ardından eklentiler.
pub fn build(settings: &Settings, plugins: &[Plugin]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = settings
        .presets
        .iter()
        .filter_map(|p| preset_entry(p, settings))
        .collect();

    for plugin in plugins {
        for pe in &plugin.entries {
            entries.push(Entry {
                id: format!("{}:{}", plugin.name, pe.id),
                label: pe.label.clone(),
                input_exts: pe
                    .extensions
                    .iter()
                    .map(|e| e.to_ascii_lowercase())
                    .collect(),
                action: Action::Plugin(pe.clone()),
            });
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    fn settings() -> Settings {
        Settings {
            presets: presets::defaults(),
            ..Settings::default()
        }
    }

    #[test]
    fn image_entries_exclude_own_format_and_skip_media() {
        let entries = build(&settings(), &[]);
        let jpg = entries.iter().find(|e| e.id == "jpg").unwrap();
        assert!(!jpg.accepts("jpg") && !jpg.accepts("JPEG"));
        assert!(jpg.accepts("png") && jpg.accepts("TIF"));
        assert!(!jpg.accepts("mp4"));
    }

    #[test]
    fn pdf_entry_covers_images_and_documents() {
        let entries = build(&settings(), &[]);
        let pdf = entries.iter().find(|e| e.id == "pdf").unwrap();
        for ext in [
            "png", "JPG", "docx", "xlsx", "pptx", "odt", "txt", "md", "html",
        ] {
            assert!(pdf.accepts(ext), "{ext}");
        }
        assert!(!pdf.accepts("pdf") && !pdf.accepts("mp4"));
        assert!(matches!(pdf.action, Action::Pdf(_)));
        // Belgelerde görüntü dönüştürme girdileri görünmez.
        assert!(
            !entries
                .iter()
                .find(|e| e.id == "jpg")
                .unwrap()
                .accepts("docx")
        );
    }

    #[test]
    fn media_entries_follow_input_groups() {
        let entries = build(&settings(), &[]);
        let mp3 = entries.iter().find(|e| e.id == "mp3").unwrap();
        assert!(
            mp3.accepts("mp4") && mp3.accepts("wav") && !mp3.accepts("mp3") && !mp3.accepts("png")
        );
        let mp4 = entries.iter().find(|e| e.id == "mp4").unwrap();
        assert!(mp4.accepts("mkv") && !mp4.accepts("mp4") && !mp4.accepts("wav"));
        assert!(matches!(mp4.action, Action::Ffmpeg(_)));
    }

    #[test]
    fn suffixed_ffmpeg_variant_stays_available_for_its_own_format() {
        let mut s = settings();
        let mut p = s.presets.iter().find(|p| p.id == "mp4").cloned().unwrap();
        p.id = "mp4-720p".into();
        p.suffix = Some("720p".into());
        s.presets.push(p);
        let entries = build(&s, &[]);
        assert!(
            entries
                .iter()
                .find(|e| e.id == "mp4-720p")
                .unwrap()
                .accepts("mp4")
        );
        assert!(
            !entries
                .iter()
                .find(|e| e.id == "mp4")
                .unwrap()
                .accepts("mp4")
        );
    }

    #[test]
    fn resizing_preset_stays_available_for_its_own_format() {
        let mut s = settings();
        let mut web = s.presets.iter().find(|p| p.id == "jpg").cloned().unwrap();
        web.id = "web-jpg".into();
        web.max_width = Some(1920);
        s.presets.push(web);
        let entries = build(&s, &[]);
        let web = entries.iter().find(|e| e.id == "web-jpg").unwrap();
        assert!(web.accepts("jpg") && web.accepts("png"));
        assert!(
            matches!(&web.action, Action::Image { options, .. } if options.max_size == Some((1920, 0)))
        );
    }
}
