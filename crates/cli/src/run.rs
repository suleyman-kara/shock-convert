//! Seçili dosyaları bir menü girdisiyle işler ve sonucu özetler.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use shock_convert_core::{Format, Options, convert_file_with, reserve_output};

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
    // Birleştirme dosyaları tek tek değil, hep birlikte işler.
    if let Action::PdfTool(job) = &entry.action
        && job.tool == crate::pdf::tools::Tool::Merge
    {
        return merge_all(entry, files, &job.suffix);
    }
    let report = Mutex::new(Report::default());
    let next = AtomicUsize::new(0);
    // ffmpeg kendi içinde çok çekirdek kullanır; üst üste fazla süreç başlatma.
    let cap = match entry.action {
        Action::Ffmpeg(_) | Action::Pdf(_) | Action::PdfTool(_) => 2,
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
    // Harici araçlara (ffmpeg, LibreOffice, tarayıcı, eklentiler) her zaman mutlak yol verilir:
    // komut satırından göreli "-y.mp4" gibi bir ad, çıktısı "-y.mp3" olunca seçenek sanılıyordu.
    let file = &absolute_input(file);
    let result = match &entry.action {
        Action::Image { format, options } => convert_image(file, *format, options),
        Action::Ffmpeg(job) => run_ffmpeg(file, job),
        Action::Pdf(job) => crate::pdf::run(file, job),
        Action::PdfTool(job) => job.run(file),
        Action::Plugin(p) => run_plugin(p, file),
    };
    match result {
        Ok(out) => {
            // Çıktı kaynağın tarihlerini taşır: Gezgin'de tarihe göre sıralama ve fotoğraf
            // arşivleri bozulmasın. PDF araçları bunu kendi yazdıkları her dosya için yapar;
            // eklentilerin çıktısına dokunulmaz.
            if !matches!(entry.action, Action::Plugin(_) | Action::PdfTool(_))
                && let Err(e) = shock_convert_core::copy_file_times(file, &out)
            {
                crate::log::line(&format!("tarih kopyalanamadı {}: {e}", out.display()));
            }
            Outcome::Done(out)
        }
        Err(e) => Outcome::Failed(e),
    }
}

fn merge_all(entry: &Entry, files: &[PathBuf], suffix: &str) -> Report {
    let mut report = Report::default();
    let mut accepted = Vec::new();
    for f in files {
        let ext = f
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        if entry.accepts(&ext) {
            accepted.push(absolute_input(f));
        } else {
            report.skipped += 1;
        }
    }
    match crate::pdf::tools::merge(&accepted, suffix) {
        Ok(out) => report.done.push(out),
        Err(e) => report
            .failed
            .push((accepted.first().cloned().unwrap_or_default(), e)),
    }
    report
}

fn absolute_input(file: &Path) -> PathBuf {
    std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf())
}

fn convert_image(file: &Path, target: Format, options: &Options) -> Result<PathBuf, String> {
    convert_file_with(file, target, options).map_err(|e| e.to_string())
}

fn run_ffmpeg(file: &Path, job: &FfmpegJob) -> Result<PathBuf, String> {
    ffmpeg::run(
        job.ffmpeg_path.as_deref(),
        &job.args,
        job.suffix.as_deref(),
        &job.output_ext,
        file,
    )
}

fn run_plugin(p: &PluginEntry, input: &Path) -> Result<PathBuf, String> {
    let reserved = reserve_output(input, Some(p.output_suffix.as_str()), &p.output_ext)
        .map_err(|e| e.to_string())?;
    let output = reserved.path();
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
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim().lines().last().unwrap_or("bilinmeyen hata");
        return Err(format!("eklenti hata verdi ({}): {msg}", out.status));
    }
    // Geçici çıktı rezerve edilirken boş oluşturulur; eklenti içini doldurmuş olmalı.
    if std::fs::metadata(output).map_or(true, |m| m.len() == 0) {
        return Err("eklenti çıktı dosyası üretmedi".into());
    }
    reserved.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_inputs_become_absolute_so_tools_never_see_a_leading_dash() {
        let abs = absolute_input(Path::new("-y.mp4"));
        assert!(abs.is_absolute(), "{abs:?}");
        assert_eq!(abs.file_name().unwrap(), "-y.mp4");
        assert!(!abs.to_string_lossy().starts_with('-'));
    }
}
