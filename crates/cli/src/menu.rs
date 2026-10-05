//! Veriye dayalı menü modeli: yerleşik formatlar ve eklenti girdileri tek listede.

use shock_convert_core::Format;

use crate::plugins::{Plugin, PluginEntry};

#[derive(Debug, Clone)]
pub enum Action {
    Builtin(Format),
    Plugin(PluginEntry),
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Komut satırındaki kimlik: `png`, `jpg` veya `<eklenti>:<girdi>`.
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

/// Dönüştürülebilen tüm girdi uzantıları.
fn all_input_exts() -> Vec<&'static str> {
    Format::ALL
        .iter()
        .flat_map(|f| f.input_exts().iter().copied())
        .collect()
}

/// Menü sırası: önce yerleşik formatlar, ardından eklentiler (yüklenme sırasıyla).
pub fn build(plugins: &[Plugin]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Format::ALL
        .iter()
        .map(|&target| Entry {
            id: target.ext().to_string(),
            label: target.label().to_string(),
            // Dosyayı kendi formatına çevirme girdisi gösterilmez.
            input_exts: all_input_exts()
                .into_iter()
                .filter(|e| !target.input_exts().contains(e))
                .map(String::from)
                .collect(),
            action: Action::Builtin(target),
        })
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

    #[test]
    fn builtin_entries_exclude_their_own_format() {
        let entries = build(&[]);
        let jpg = entries.iter().find(|e| e.id == "jpg").unwrap();
        assert!(!jpg.accepts("jpg") && !jpg.accepts("JPEG"));
        assert!(jpg.accepts("png") && jpg.accepts("TIF"));
        assert_eq!(entries.len(), Format::ALL.len());
    }
}
