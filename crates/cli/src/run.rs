//! Seçili dosyaları bir menü girdisiyle işler ve sonucu özetler.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use shock_convert_core::{Format, Options, convert_file_with, unique_output_path};

use crate::ffmpeg;
use crate::menu::{Action, Entry, FfmpegJob};
use crate::plugins::PluginEntry;

#[derive(Debug, Default)]
pub struct Report {
    pub done: Vec<PathBuf>,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
}

impl Report {
    pub fn summary(&self, entry: &Entry) -> String {
        let mut parts = Vec::new();
        if !self.done.is_empty() {
            parts.push(format!(
                "{} dosya → {} tamamlandı",
                self.done.len(),
                entry.label
            ));
        }
        if self.skipped > 0 {
            parts.push(format!(
                "{} dosya atlandı (zaten bu formatta/desteklenmiyor)",
                self.skipped
            ));
        }
        for (path, err) in self.failed.iter().take(3) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            parts.push(format!("Hata — {name}: {err}"));
        }
        if self.failed.len() > 3 {
            parts.push(format!("… ve {} hata daha", self.failed.len() - 3));
        }
        parts.join("\n")
    }
}

/// Dosyaları paralel işler. Sonuç sırası girdi sırasından bağımsızdır.
pub fn run_entry(entry: &Entry, files: &[PathBuf]) -> Report {
    let report = Mutex::new(Report::default());
    let next = AtomicUsize::new(0);
    // ffmpeg kendi içinde çok çekirdek kullanır; üst üste fazla süreç başlatma.
    let cap = match entry.action {
        Action::Ffmpeg(_) => 2,
        _ => usize::MAX,
    };
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .min(cap)
        .min(files.len().max(1));

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(file) = files.get(i) else { break };
                    let outcome = process(entry, file);
                    let mut r = report.lock().unwrap();
                    match outcome {
                        Outcome::Done(p) => r.done.push(p),
                        Outcome::Skipped => r.skipped += 1,
                        Outcome::Failed(e) => r.failed.push((file.clone(), e)),
                    }
                }
            });
        }
    });
    report.into_inner().unwrap()
}

enum Outcome {
    Done(PathBuf),
    Skipped,
    Failed(String),
}

fn process(entry: &Entry, file: &Path) -> Outcome {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !entry.accepts(&ext) {
        return Outcome::Skipped;
    }
    let result = match &entry.action {
        Action::Image { format, options } => convert_image(file, *format, options),
        Action::Ffmpeg(job) => run_ffmpeg(file, job),
        Action::Plugin(p) => run_plugin(p, file),
    };
    match result {
        Ok(out) => Outcome::Done(out),
        Err(e) => Outcome::Failed(e),
    }
}

fn convert_image(file: &Path, target: Format, options: &Options) -> Result<PathBuf, String> {
    convert_file_with(file, target, options).map_err(|e| e.to_string())
}

fn run_ffmpeg(file: &Path, job: &FfmpegJob) -> Result<PathBuf, String> {
    ffmpeg::run(
        job.ffmpeg_path.as_deref(),
        &job.args,
        &job.suffix,
        &job.output_ext,
        file,
    )
}

fn run_plugin(p: &PluginEntry, input: &Path) -> Result<PathBuf, String> {
    let output =
        unique_output_path(input, &p.output_suffix, &p.output_ext).map_err(|e| e.to_string())?;
    let args: Vec<String> = p
        .command
        .iter()
        .map(|a| {
            a.replace("{input}", &input.to_string_lossy())
                .replace("{output}", &output.to_string_lossy())
                .replace("{plugin_dir}", &p.plugin_dir.to_string_lossy())
        })
        .collect();

    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|e| format!("`{}` başlatılamadı: {e}", args[0]))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&output);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim().lines().last().unwrap_or("bilinmeyen hata");
        return Err(format!("eklenti hata verdi ({}): {msg}", out.status));
    }
    // Çıktı adı rezerve edilirken boş dosya oluşturulur; eklenti içini doldurmuş olmalı.
    if std::fs::metadata(&output).map_or(true, |m| m.len() == 0) {
        let _ = std::fs::remove_file(&output);
        return Err("eklenti çıktı dosyası üretmedi".into());
    }
    Ok(output)
}
