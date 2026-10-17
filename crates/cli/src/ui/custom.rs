//! Özel dönüştürme penceresi (#24): sağ tıkla seçilen görüntüler için ayarlar bir kerelik girilir,
//! dönüştürme pencerede ilerlemesiyle izlenir. Ayarlar bellekte bir preset'e çevrilip mevcut
//! dönüştürme hattından (`run::process`) geçirilir; istenirse kalıcı preset olarak kaydedilir.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shock_convert_core::{Format, Metadata};

use super::{App, store};
use crate::menu::{Action, Entry};
use crate::presets::{self, RawPreset};
use crate::run::Outcome;

/// Pencerede seçilen tek seferlik ayarlar.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Spec {
    pub suffix: String,
    pub format: String,
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub background: String,
    /// keep | strip-location | strip ("" = ayarlardaki)
    pub metadata: String,
}

fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Ayarları doğrulanmamış bir preset'e çevirir (kaydetme ve çalıştırma aynı yoldan geçer).
fn to_raw(s: &Spec, id: &str, label: &str) -> RawPreset {
    RawPreset {
        id: id.into(),
        label: label.into(),
        kind: "image".into(),
        output: s.format.clone(),
        input: vec!["image".into()],
        suffix: Some(s.suffix.trim().to_string()).filter(|x| !x.is_empty()),
        quality: s.quality,
        max_width: s.max_width,
        max_height: s.max_height,
        background: Some(s.background.clone()).filter(|b| !b.is_empty()),
    }
}

/// Tek seferlik menü girdisi: ayarlardaki tarih ve EXIF tercihleri geçerli kalır.
fn entry_for(s: &Spec) -> Result<Entry, String> {
    let preset = to_raw(s, "custom", "Custom convert").into_preset()?;
    let (settings, _) = presets::load();
    let mut entry = crate::menu::preset_entry(&preset, &settings)
        .ok_or("there is no file type to convert with these settings")?;
    // Özel dönüştürmede dosya kendi biçimine de çevrilebilir (ör. JPG'yi küçültüp yine JPG).
    entry.input_exts = preset.input.clone();
    if let Action::Image { options, .. } = &mut entry.action
        && let Some(m) = Metadata::parse(&s.metadata)
    {
        options.metadata = m;
    }
    Ok(entry)
}

#[derive(Debug, Default, Clone, Serialize)]
struct JobState {
    running: bool,
    finished: bool,
    cancelled: bool,
    total: usize,
    /// İşlenen dosya sayısı (sonuç bekleyen hariç).
    index: usize,
    current: Option<String>,
    /// (girdi adı, çıktı yolu)
    done: Vec<(String, String)>,
    /// (girdi adı, hata)
    failed: Vec<(String, String)>,
    skipped: usize,
    label: String,
}

struct Custom {
    files: Vec<PathBuf>,
    job: Mutex<JobState>,
    busy: AtomicBool,
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn last_path() -> PathBuf {
    crate::log::path()
        .parent()
        .map(|d| d.join("custom-last.json"))
        .unwrap_or_else(|| std::env::temp_dir().join("shock-convert-custom-last.json"))
}

impl Custom {
    fn state(&self) -> Value {
        let files: Vec<Value> = self
            .files
            .iter()
            .map(|f| {
                let ext = ext_of(f);
                let image = Format::from_ext(&ext).is_some();
                json!({ "name": name_of(f), "ext": ext, "image": image })
            })
            .collect();
        let last: Value = std::fs::read_to_string(last_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let (settings, _) = presets::load();
        let formats: Vec<Value> = Format::ALL
            .iter()
            .map(|f| json!({ "ext": f.ext(), "label": f.label() }))
            .collect();
        json!({
            "files": files,
            "last": last,
            "formats": formats,
            "metadata": settings.metadata.as_str(),
            "version": env!("CARGO_PKG_VERSION"),
        })
    }

    fn start(self: &Arc<Self>, body: &[u8]) -> (&'static str, Value) {
        let spec: Spec = match serde_json::from_slice(body) {
            Ok(s) => s,
            Err(e) => return super::error("400 Bad Request", e.to_string()),
        };
        let entry = match entry_for(&spec) {
            Ok(e) => e,
            Err(e) => return super::error("400 Bad Request", e),
        };
        if self.busy.swap(true, Ordering::SeqCst) {
            return super::error("409 Conflict", "a conversion is already running");
        }
        let _ = std::fs::write(last_path(), serde_json::to_vec(&spec).unwrap_or_default());
        *self.job.lock().unwrap() = JobState {
            running: true,
            total: self.files.len(),
            label: entry.label.clone(),
            ..JobState::default()
        };
        let me = Arc::clone(self);
        std::thread::spawn(move || me.run(&entry));
        ("200 OK", json!({ "ok": true }))
    }

    fn run(&self, entry: &Entry) {
        crate::job::start();
        crate::log::line(&format!(
            "custom convert: {} files, {}",
            self.files.len(),
            entry.label
        ));
        for (i, file) in self.files.iter().enumerate() {
            if crate::job::cancelled() {
                break;
            }
            self.update(|j| {
                j.index = i;
                j.current = Some(name_of(file));
            });
            let outcome = crate::run::process(entry, file);
            self.update(|j| {
                match outcome {
                    Outcome::Done(p) => j.done.push((name_of(file), p.display().to_string())),
                    Outcome::Skipped => j.skipped += 1,
                    Outcome::Failed(e) => j.failed.push((name_of(file), e)),
                }
                j.index = i + 1;
            });
        }
        let cancelled = crate::job::cancelled();
        self.update(|j| {
            j.running = false;
            j.finished = true;
            j.cancelled = cancelled;
            j.current = None;
        });
        crate::log::line(&format!(
            "custom convert finished: {:?}",
            self.job.lock().unwrap()
        ));
        self.busy.store(false, Ordering::SeqCst);
    }

    fn update(&self, f: impl FnOnce(&mut JobState)) {
        f(&mut self.job.lock().unwrap());
    }

    fn job_json(&self) -> Value {
        serde_json::to_value(&*self.job.lock().unwrap()).unwrap_or(Value::Null)
    }

    fn save_preset(&self, body: &[u8]) -> (&'static str, Value) {
        #[derive(Deserialize)]
        struct Req {
            label: String,
            spec: Spec,
        }
        let req: Req = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => return super::error("400 Bad Request", e.to_string()),
        };
        let label = req.label.trim();
        if label.is_empty() {
            return super::error("400 Bad Request", "a name is required");
        }
        let mut values = match store::Values::current() {
            Ok(v) => v,
            Err(e) => return super::error("500 Internal Server Error", e),
        };
        let mut id = slug(label);
        let builtin = presets::defaults().iter().any(|p| p.id == id);
        if id.is_empty() || builtin {
            id = format!("custom-{id}").trim_end_matches('-').to_string();
        }
        let raw = to_raw(&req.spec, &id, label);
        values.presets.retain(|p| p.id != id);
        values.presets.push(raw);
        if let Err(errors) = store::save(&values) {
            let text: Vec<String> = errors
                .into_iter()
                .map(|(k, e)| format!("{k}: {e}"))
                .collect();
            return super::error("400 Bad Request", text.join(" · "));
        }
        let menu_error = super::settings::reregister_menu();
        (
            "200 OK",
            json!({ "ok": true, "id": id, "menu_error": menu_error }),
        )
    }
}

/// Menü adından kimlik: "JPG for web" → "jpg-for-web". Türkçe harfler sadeleştirilir.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let c = match c {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' | 'i' => 'i',
            'ö' => 'o',
            'ş' => 's',
            'ü' => 'u',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(40).collect()
}

struct Shared(Arc<Custom>);

impl App for Shared {
    fn page(&self) -> &'static str {
        include_str!("custom.html")
    }

    fn window_size(&self) -> (u32, u32) {
        (760, 760)
    }

    fn api(&self, method: &str, path: &str, body: &[u8]) -> Option<(&'static str, Value)> {
        let c = &self.0;
        Some(match (method, path) {
            ("GET", "/api/state") => ("200 OK", c.state()),
            ("POST", "/api/run") => c.start(body),
            ("GET", "/api/job") => ("200 OK", c.job_json()),
            ("POST", "/api/cancel") => {
                if c.busy.load(Ordering::SeqCst) {
                    crate::job::cancel();
                }
                ("200 OK", json!({ "ok": true }))
            }
            ("POST", "/api/reveal") => {
                #[derive(Deserialize)]
                struct Req {
                    index: usize,
                }
                // Yalnızca bu işin ürettiği dosyalar gösterilebilir.
                let out = serde_json::from_slice::<Req>(body).ok().and_then(|r| {
                    c.job
                        .lock()
                        .unwrap()
                        .done
                        .get(r.index)
                        .map(|(_, o)| PathBuf::from(o))
                });
                match out {
                    Some(p) => {
                        super::reveal(&p);
                        ("200 OK", json!({ "ok": true }))
                    }
                    None => super::error("404 Not Found", "no such output"),
                }
            }
            ("POST", "/api/save-preset") => c.save_preset(body),
            _ => return None,
        })
    }

    fn busy(&self) -> bool {
        self.0.busy.load(Ordering::SeqCst)
    }

    fn on_exit(&self, window_open: bool) {
        // Pencere iş sürerken kapatıldıysa sonucu bildirimle söyle.
        let j = self.0.job.lock().unwrap().clone();
        if j.finished && !window_open {
            let mut parts = vec![format!("{} files → {} done", j.done.len(), j.label)];
            if !j.failed.is_empty() {
                parts.push(format!("{} errors", j.failed.len()));
            }
            if j.cancelled {
                parts.push("cancelled".into());
            }
            crate::platform::notify("Shock Convert", &parts.join(" · "));
        }
    }
}

/// Özel dönüştürme penceresini açar; pencere kapanıp iş bitene kadar bekler.
pub fn open(files: Vec<PathBuf>, print_url: bool) -> Result<(), String> {
    let files: Vec<PathBuf> = files
        .into_iter()
        .map(|f| std::path::absolute(&f).unwrap_or(f))
        .collect();
    let app = Custom {
        files,
        job: Mutex::new(JobState::default()),
        busy: AtomicBool::new(false),
    };
    super::serve(Arc::new(Shared(Arc::new(app))), print_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_become_valid_presets() {
        let image = Spec {
            format: "jpg".into(),
            quality: Some(80),
            max_width: Some(1920),
            suffix: "web".into(),
            ..Spec::default()
        };
        let p = to_raw(&image, "x", "X").into_preset().unwrap();
        assert_eq!(
            (p.output.as_str(), p.quality, p.suffix.as_deref()),
            ("jpg", Some(80), Some("web"))
        );

        let bad = Spec {
            format: "exe".into(),
            ..Spec::default()
        };
        assert!(to_raw(&bad, "b", "B").into_preset().is_err());
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Web için JPG (1920 px)"), "web-icin-jpg-1920-px");
        assert_eq!(slug("JPG for web"), "jpg-for-web");
    }
}
