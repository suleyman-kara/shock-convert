//! Seçili dosyaları bir menü girdisiyle işler ve sonucu özetler.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use shock_convert_core::{Format, Options, convert_file_with, reserve_output};

use crate::menu::{Action, Entry};
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
                "{} {} → {} done",
                self.done.len(),
                files(self.done.len()),
                entry.label
            ));
        }
        if self.skipped > 0 {
            parts.push(format!(
                "{} {} skipped (already in this format or not supported)",
                self.skipped,
                files(self.skipped)
            ));
        }
        for (path, err) in self.failed.iter().take(3) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            parts.push(format!("Error — {name}: {err}"));
        }
        if self.failed.len() > 3 {
            parts.push(format!("… and {} more errors", self.failed.len() - 3));
        }
        parts.join("\n")
    }
}

fn files(n: usize) -> &'static str {
    if n == 1 { "file" } else { "files" }
}

/// Dosyaları paralel işler. Sonuç sırası girdi sırasından bağımsızdır.
pub fn run_entry(entry: &Entry, files: &[PathBuf]) -> Report {
    let report = Mutex::new(Report::default());
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
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

pub(crate) enum Outcome {
    Done(PathBuf),
    Skipped,
    Failed(String),
}

pub(crate) fn process(entry: &Entry, file: &Path) -> Outcome {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !entry.accepts(&ext) {
        return Outcome::Skipped;
    }
    // Eklentilere her zaman mutlak yol verilir: komut satırından göreli "-y.png" gibi bir ad
    // seçenek sanılabilir.
    let file = &absolute_input(file);
    let result = match &entry.action {
        Action::Image { format, options } => convert_image(file, *format, options),
        Action::Plugin(p) => run_plugin(p, file),
    };
    match result {
        Ok(out) => {
            // Çıktı kaynağın tarihlerini taşır: Gezgin'de tarihe göre sıralama ve fotoğraf
            // arşivleri bozulmasın. Eklentilerin çıktısına dokunulmaz.
            if entry.keep_dates
                && !matches!(entry.action, Action::Plugin(_))
                && let Err(e) = shock_convert_core::copy_file_times(file, &out)
            {
                crate::log::line(&format!("could not copy dates {}: {e}", out.display()));
            }
            Outcome::Done(out)
        }
        Err(e) => Outcome::Failed(e),
    }
}

fn absolute_input(file: &Path) -> PathBuf {
    std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf())
}

fn convert_image(file: &Path, target: Format, options: &Options) -> Result<PathBuf, String> {
    convert_file_with(file, target, options).map_err(|e| e.to_string())
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
        .map_err(|e| format!("could not start `{}`: {e}", args[0]))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim().lines().last().unwrap_or("unknown error");
        return Err(format!("plugin failed ({}): {msg}", out.status));
    }
    // Geçici çıktı rezerve edilirken boş oluşturulur; eklenti içini doldurmuş olmalı.
    if std::fs::metadata(output).map_or(true, |m| m.len() == 0) {
        return Err("plugin produced no output file".into());
    }
    reserved.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_inputs_become_absolute_so_tools_never_see_a_leading_dash() {
        let abs = absolute_input(Path::new("-y.png"));
        assert!(abs.is_absolute(), "{abs:?}");
        assert_eq!(abs.file_name().unwrap(), "-y.png");
        assert!(!abs.to_string_lossy().starts_with('-'));
    }
}
