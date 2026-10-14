//! Özel dönüştürme penceresi (#24): sağ tıkla seçilen dosyalar için ayarlar bir kerelik girilir,
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
use crate::presets::{self, AUDIO, DOCUMENT, Kind, RawPreset, VIDEO};
use crate::run::Outcome;

/// Pencerede seçilen tek seferlik ayarlar.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Spec {
    /// image | media | pdf-tool | to-pdf
    pub kind: String,
    pub suffix: String,
    // Görüntü
    pub format: String,
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub background: String,
    /// keep | strip-location | strip ("" = ayarlardaki)
    pub metadata: String,
    // Ses / video
    pub output: String,
    /// Video yüksekliği (ör. 720); yoksa özgün boyut. Asla büyütülmez.
    pub height: Option<u32>,
    /// high | normal | small
    pub level: String,
    /// Ses bit hızı (kbit/sn)
    pub audio_kbps: Option<u32>,
    pub no_audio: bool,
    /// Kırpma: başlangıç ve bitiş ("90", "1:30", "00:01:30.5")
    pub start: String,
    pub end: String,
    pub extra: String,
    // PDF işlemi
    pub tool: String,
    pub dpi: Option<u32>,
}

const AUDIO_OUT: &[&str] = &["mp3", "m4a", "ogg", "opus", "flac", "wav"];
const VIDEO_OUT: &[&str] = &["mp4", "webm", "mkv", "gif"];

fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Dosyanın hangi özel dönüştürme türlerine uyduğu.
fn kinds_for(ext: &str) -> Vec<&'static str> {
    if Format::from_ext(ext).is_some() {
        vec!["image", "to-pdf"]
    } else if AUDIO.contains(&ext) || VIDEO.contains(&ext) {
        vec!["media"]
    } else if ext == "pdf" {
        vec!["pdf-tool"]
    } else if DOCUMENT.contains(&ext) {
        vec!["to-pdf"]
    } else {
        vec![]
    }
}

/// Zaman değeri yalnızca rakam, `:` ve `.` içerebilir (ffmpeg'e seçenek olarak gitmesin).
fn valid_time(t: &str) -> bool {
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
}

/// Tırnakları tanıyan basit bölücü: `-metadata 'title=Benim video'` → 2 argüman.
fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut has) = (Vec::new(), String::new(), None, false);
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                has = true;
            }
            None if c.is_whitespace() => {
                if !cur.is_empty() || has {
                    out.push(std::mem::take(&mut cur));
                }
                has = false;
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() || has {
        out.push(cur);
    }
    out
}

/// Ses/video ayarlarından ffmpeg argümanları.
fn media_args(s: &Spec) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();
    let mut push = |xs: &[&str]| a.extend(xs.iter().map(|x| x.to_string()));
    for (flag, t) in [("-ss", s.start.trim()), ("-to", s.end.trim())] {
        if !t.is_empty() {
            if !valid_time(t) {
                return Err(format!("geçersiz zaman: {t} (ör. 90, 1:30 ya da 00:01:30)"));
            }
            push(&[flag, t]);
        }
    }
    let kbps = s.audio_kbps.unwrap_or(192).clamp(32, 512);
    let br = format!("{kbps}k");
    let (x264, vp9) = match s.level.as_str() {
        "high" => ("20", "28"),
        "small" => ("28", "38"),
        _ => ("23", "32"),
    };
    let scale = s
        .height
        .filter(|h| (100..=4320).contains(h))
        .map(|h| format!("scale=-2:min(ih\\,{h})"));
    let out = s.output.as_str();
    match out {
        "mp3" => push(&[
            "-vn",
            "-c:a",
            "libmp3lame",
            "-b:a",
            &br,
            "-id3v2_version",
            "3",
            "-write_id3v1",
            "1",
        ]),
        "m4a" => push(&["-vn", "-c:a", "aac", "-b:a", &br]),
        "ogg" => push(&["-vn", "-c:a", "libvorbis", "-b:a", &br]),
        "opus" => push(&["-vn", "-c:a", "libopus", "-b:a", &br]),
        "flac" => push(&["-vn", "-c:a", "flac"]),
        "wav" => push(&["-vn", "-c:a", "pcm_s16le"]),
        "mp4" | "mkv" | "webm" => {
            if let Some(f) = &scale {
                push(&["-vf", f]);
            }
            if out == "webm" {
                push(&["-c:v", "libvpx-vp9", "-crf", vp9, "-b:v", "0"]);
            } else {
                push(&["-c:v", "libx264", "-crf", x264, "-preset", "medium"]);
            }
            if s.no_audio {
                push(&["-an"]);
            } else if out == "webm" {
                push(&["-c:a", "libopus", "-b:a", &br]);
            } else {
                push(&["-c:a", "aac", "-b:a", &br]);
            }
            if out == "mp4" {
                push(&["-movflags", "+faststart"]);
            }
        }
        "gif" => {
            let size = match s.height.filter(|h| (100..=2160).contains(h)) {
                Some(h) => format!("scale=-2:min(ih\\,{h}):flags=lanczos"),
                None => "scale=480:-1:flags=lanczos".into(),
            };
            let vf = format!("fps=12,{size},split[a][b];[a]palettegen[p];[b][p]paletteuse");
            push(&["-vf", &vf, "-an"]);
        }
        "" => return Err("çıktı biçimi seçilmedi".into()),
        other => return Err(format!("desteklenmeyen çıktı: {other}")),
    }
    a.extend(split_args(&s.extra));
    Ok(a)
}

/// Ayarları doğrulanmamış bir preset'e çevirir (kaydetme ve çalıştırma aynı yoldan geçer).
fn to_raw(s: &Spec, id: &str, label: &str) -> Result<RawPreset, String> {
    let suffix = Some(s.suffix.trim().to_string()).filter(|x| !x.is_empty());
    let mut p = RawPreset {
        id: id.into(),
        label: label.into(),
        kind: Kind::Image,
        output: String::new(),
        input: vec![],
        suffix,
        quality: None,
        max_width: None,
        max_height: None,
        background: None,
        args: vec![],
        tool: None,
        dpi: None,
    };
    match s.kind.as_str() {
        "image" => {
            p.output = s.format.clone();
            p.input = vec!["image".into()];
            p.quality = s.quality;
            p.max_width = s.max_width;
            p.max_height = s.max_height;
            p.background = Some(s.background.clone()).filter(|b| !b.is_empty());
        }
        "media" => {
            p.kind = Kind::Ffmpeg;
            p.output = s.output.clone();
            p.input = if AUDIO_OUT.contains(&s.output.as_str()) {
                vec!["media".into()]
            } else {
                vec!["video".into()]
            };
            p.args = media_args(s)?;
        }
        "pdf-tool" => {
            p.kind = Kind::PdfTool;
            p.tool = Some(s.tool.clone());
            p.input = vec!["pdf".into()];
            p.output = match s.tool.as_str() {
                "to-images" => s.format.clone(),
                "to-text" => "txt".into(),
                _ => "pdf".into(),
            };
            p.quality = s.quality;
            p.max_width = s.max_width;
            p.dpi = s.dpi;
            if p.suffix.is_none() {
                p.suffix = Some(
                    match s.tool.as_str() {
                        "split" | "to-images" => "sayfa",
                        "compress" => "küçük",
                        "merge" => "birleşik",
                        "to-text" => "",
                        _ => "döndürülmüş",
                    }
                    .into(),
                )
                .filter(|x: &String| !x.is_empty());
            }
        }
        "to-pdf" => {
            p.kind = Kind::Pdf;
            p.output = "pdf".into();
            p.input = vec!["image".into(), "document".into()];
        }
        other => return Err(format!("bilinmeyen tür: {other}")),
    }
    Ok(p)
}

/// Tek seferlik menü girdisi: ayarlardaki araç yolları ve tarih tercihleri geçerli kalır.
fn entry_for(s: &Spec) -> Result<Entry, String> {
    let preset = to_raw(s, "custom", "Özel dönüştürme")?.into_preset()?;
    let (settings, _) = presets::load();
    let mut entry = crate::menu::preset_entry(&preset, &settings)
        .ok_or("bu ayarlarla dönüştürülecek dosya türü yok")?;
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
                json!({ "name": name_of(f), "ext": ext, "kinds": kinds_for(&ext) })
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
            "audio_out": AUDIO_OUT,
            "video_out": VIDEO_OUT,
            "audio_in": AUDIO,
            "metadata": settings.metadata.as_str(),
            "ffmpeg": crate::ffmpeg::locate(settings.ffmpeg_path.as_deref()).is_some(),
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
            return super::error("409 Conflict", "bir dönüştürme zaten sürüyor");
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
        crate::job::start(true);
        crate::log::line(&format!(
            "özel dönüştürme: {} dosya, {}",
            self.files.len(),
            entry.label
        ));
        let merge =
            matches!(&entry.action, Action::PdfTool(j) if j.tool == crate::pdf::tools::Tool::Merge);
        if merge {
            // Birleştirme dosyaları birlikte işler.
            self.update(|j| j.current = Some(format!("{} dosya", self.files.len())));
            let report = crate::run::run_entry(entry, &self.files);
            self.update(|j| {
                j.done.extend(
                    report
                        .done
                        .iter()
                        .map(|p| (String::new(), p.display().to_string())),
                );
                j.failed
                    .extend(report.failed.iter().map(|(f, e)| (name_of(f), e.clone())));
                j.skipped += report.skipped;
                j.index = j.total;
            });
        } else {
            for (i, file) in self.files.iter().enumerate() {
                if crate::job::cancelled() {
                    break;
                }
                crate::job::set_progress(None);
                self.update(|j| {
                    j.index = i;
                    j.current = Some(name_of(file));
                });
                let outcome = crate::run::process(entry, file);
                self.update(|j| {
                    match outcome {
                        Outcome::Done(p) => j.done.push((name_of(file), p.display().to_string())),
                        Outcome::Skipped => j.skipped += 1,
                        Outcome::Failed(e) if e == crate::job::CANCELLED => {}
                        Outcome::Failed(e) => j.failed.push((name_of(file), e)),
                    }
                    j.index = i + 1;
                });
            }
        }
        let cancelled = crate::job::cancelled();
        self.update(|j| {
            j.running = false;
            j.finished = true;
            j.cancelled = cancelled;
            j.current = None;
        });
        crate::log::line(&format!(
            "özel dönüştürme bitti: {:?}",
            self.job.lock().unwrap()
        ));
        self.busy.store(false, Ordering::SeqCst);
    }

    fn update(&self, f: impl FnOnce(&mut JobState)) {
        f(&mut self.job.lock().unwrap());
    }

    fn job_json(&self) -> Value {
        let j = self.job.lock().unwrap().clone();
        let mut v = serde_json::to_value(&j).unwrap_or(Value::Null);
        v["progress"] = crate::job::progress().map_or(Value::Null, |p| json!(p));
        v
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
            return super::error("400 Bad Request", "ad gerekli");
        }
        let mut values = match store::Values::current() {
            Ok(v) => v,
            Err(e) => return super::error("500 Internal Server Error", e),
        };
        let mut id = slug(label);
        let builtin = presets::defaults().iter().any(|p| p.id == id);
        if id.is_empty() || builtin {
            id = format!("ozel-{id}").trim_end_matches('-').to_string();
        }
        let raw = match to_raw(&req.spec, &id, label) {
            Ok(r) => r,
            Err(e) => return super::error("400 Bad Request", e),
        };
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

/// Menü adından kimlik: "Web için JPG" → "web-icin-jpg".
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
        (760, 820)
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
                    None => super::error("404 Not Found", "çıktı yok"),
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
            let mut parts = vec![format!("{} dosya → {} tamamlandı", j.done.len(), j.label)];
            if !j.failed.is_empty() {
                parts.push(format!("{} hata", j.failed.len()));
            }
            if j.cancelled {
                parts.push("iptal edildi".into());
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

    fn media(output: &str) -> Spec {
        Spec {
            kind: "media".into(),
            output: output.into(),
            ..Spec::default()
        }
    }

    #[test]
    fn media_args_for_common_choices() {
        let mut s = media("mp4");
        s.height = Some(720);
        s.level = "small".into();
        s.start = "1:30".into();
        s.end = "2:00".into();
        let a = media_args(&s).unwrap().join(" ");
        assert!(
            a.starts_with("-ss 1:30 -to 2:00 -vf scale=-2:min(ih\\,720)"),
            "{a}"
        );
        assert!(
            a.contains("-crf 28") && a.contains("-c:a aac") && a.ends_with("+faststart"),
            "{a}"
        );

        let mut s = media("webm");
        s.no_audio = true;
        let a = media_args(&s).unwrap().join(" ");
        assert!(
            a.contains("libvpx-vp9 -crf 32 -b:v 0") && a.ends_with("-an"),
            "{a}"
        );

        let mut s = media("mp3");
        s.audio_kbps = Some(320);
        s.extra = "-metadata 'title=Benim şarkı'".into();
        let a = media_args(&s).unwrap();
        assert!(a.contains(&"320k".to_string()) && a.contains(&"-vn".to_string()));
        assert_eq!(&a[a.len() - 2..], ["-metadata", "title=Benim şarkı"]);
    }

    #[test]
    fn rejects_unsafe_times_and_unknown_outputs() {
        let mut s = media("mp3");
        s.start = "-i /etc/passwd".into();
        assert!(media_args(&s).is_err());
        assert!(media_args(&media("exe")).is_err());
        assert!(media_args(&media("")).is_err());
    }

    #[test]
    fn specs_become_valid_presets() {
        let image = Spec {
            kind: "image".into(),
            format: "jpg".into(),
            quality: Some(80),
            max_width: Some(1920),
            suffix: "web".into(),
            ..Spec::default()
        };
        let p = to_raw(&image, "x", "X").unwrap().into_preset().unwrap();
        assert_eq!(
            (p.output.as_str(), p.quality, p.suffix.as_deref()),
            ("jpg", Some(80), Some("web"))
        );

        let pdf = Spec {
            kind: "pdf-tool".into(),
            tool: "to-images".into(),
            format: "jpg".into(),
            dpi: Some(200),
            ..Spec::default()
        };
        let p = to_raw(&pdf, "y", "Y").unwrap().into_preset().unwrap();
        assert_eq!(
            (p.output.as_str(), p.dpi, p.suffix.as_deref()),
            ("jpg", Some(200), Some("sayfa"))
        );

        let text = Spec {
            kind: "pdf-tool".into(),
            tool: "to-text".into(),
            ..Spec::default()
        };
        let p = to_raw(&text, "z", "Z").unwrap().into_preset().unwrap();
        assert_eq!((p.output.as_str(), p.suffix), ("txt", None));

        let bad = Spec {
            kind: "image".into(),
            format: "exe".into(),
            ..Spec::default()
        };
        assert!(to_raw(&bad, "b", "B").unwrap().into_preset().is_err());
    }

    #[test]
    fn file_kinds_and_slugs() {
        assert_eq!(kinds_for("png"), ["image", "to-pdf"]);
        assert_eq!(kinds_for("mp4"), ["media"]);
        assert_eq!(kinds_for("pdf"), ["pdf-tool"]);
        assert_eq!(kinds_for("docx"), ["to-pdf"]);
        assert!(kinds_for("exe").is_empty());
        assert_eq!(slug("Web için JPG (1920 px)"), "web-icin-jpg-1920-px");
        assert_eq!(split_args(r#"-a "b c" '' d"#), ["-a", "b c", "", "d"]);
    }
}
