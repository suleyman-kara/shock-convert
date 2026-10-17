//! Veriye dayalı menü modeli: preset'ler ve eklenti girdileri tek listede.

use shock_convert_core::{Format, Options};

use crate::plugins::{Plugin, PluginEntry};
use crate::presets::{Preset, Settings};

#[derive(Debug, Clone)]
pub enum Action {
    Image { format: Format, options: Options },
    Plugin(PluginEntry),
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Komut satırındaki kimlik: `png`, `web-jpg` veya `<eklenti>:<girdi>`.
    pub id: String,
    pub label: String,
    /// Bu girdinin menüde görüneceği dosya uzantıları (küçük harf, noktasız).
    pub input_exts: Vec<String>,
    pub action: Action,
    /// Çıktı kaynağın tarihlerini taşısın mı (ayarlardan).
    pub keep_dates: bool,
}

impl Entry {
    pub fn accepts(&self, ext: &str) -> bool {
        let ext = ext.to_ascii_lowercase();
        self.input_exts.contains(&ext)
    }
}

pub(crate) fn preset_entry(p: &Preset, s: &Settings) -> Option<Entry> {
    let format = Format::from_ext(&p.output)?;
    let mut input_exts = p.input.clone();
    // Dosyayı kendi formatına çevirme girdisi gösterilmez. Ama küçültme yapan ya da kendi
    // `suffix`'ini tanımlayan preset bir "varyant"tır (örn. web için JPG) ve kendi formatında da anlamlıdır.
    let resizes = p.max_width.is_some() || p.max_height.is_some();
    if !resizes && p.suffix.is_none() {
        input_exts.retain(|e| !format.input_exts().contains(&e.as_str()));
    }
    if input_exts.is_empty() {
        return None;
    }

    let defaults = Options::default();
    let action = Action::Image {
        format,
        options: Options {
            jpeg_quality: p.quality.unwrap_or(defaults.jpeg_quality),
            max_size: resizes.then(|| (p.max_width.unwrap_or(0), p.max_height.unwrap_or(0))),
            background: p.background.unwrap_or(defaults.background),
            suffix: p.suffix.clone(),
            metadata: s.metadata,
        },
    };
    Some(Entry {
        id: p.id.clone(),
        label: p.label.clone(),
        input_exts,
        action,
        keep_dates: s.keep_dates,
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
                // Eklentinin çıktısına dokunulmaz.
                keep_dates: false,
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
    fn image_entries_exclude_own_format_and_non_images() {
        let entries = build(&settings(), &[]);
        let jpg = entries.iter().find(|e| e.id == "jpg").unwrap();
        assert!(!jpg.accepts("jpg") && !jpg.accepts("JPEG"));
        assert!(jpg.accepts("png") && jpg.accepts("TIF"));
        for other in ["mp4", "pdf", "docx", "txt"] {
            assert!(entries.iter().all(|e| !e.accepts(other)), "{other}");
        }
    }

    #[test]
    fn suffixed_preset_stays_available_for_its_own_format() {
        let mut s = settings();
        let mut p = s.presets.iter().find(|p| p.id == "png").cloned().unwrap();
        p.id = "png-copy".into();
        p.suffix = Some("copy".into());
        s.presets.push(p);
        let entries = build(&s, &[]);
        let find = |id: &str| entries.iter().find(|e| e.id == id).unwrap();
        assert!(find("png-copy").accepts("png"));
        assert!(!find("png").accepts("png"));
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
