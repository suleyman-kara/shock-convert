//! ffmpeg ile ses/video dönüştürme. ffmpeg paketlenmez; sırayla yapılandırılan yol,
//! kurulum klasöründeki `ffmpeg\ffmpeg.exe` / `ffmpeg.exe`, sonra `PATH` aranır.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use shock_convert_core::unique_output_path;

const INSTALL_HINT: &str = "ffmpeg bulunamadı. Kurmak için: winget install Gyan.FFmpeg (sonra Shock Convert'i tekrar deneyin) \
     ya da presets.toml içinde `ffmpeg_path` verin.";

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

pub fn locate(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = configured.filter(|p| p.is_file()) {
        return Some(p.to_path_buf());
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for cand in [dir.join("ffmpeg").join(exe_name()), dir.join(exe_name())] {
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(exe_name()))
            .find(|c| c.is_file())
    })
}

/// `ffmpeg -i <girdi> <ayarlar> <çıktı>`. `-y`: çıktı adı önceden (boş dosya olarak) rezerve edilir.
pub fn build_args(input: &Path, output: &Path, preset_args: &[String]) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    a.push(input.as_os_str().to_owned());
    a.extend(preset_args.iter().map(OsString::from));
    a.push(output.as_os_str().to_owned());
    a
}

pub fn run(
    ffmpeg: Option<&Path>,
    preset_args: &[String],
    suffix: &str,
    ext: &str,
    input: &Path,
) -> Result<PathBuf, String> {
    let ffmpeg = locate(ffmpeg).ok_or(INSTALL_HINT)?;
    let output = unique_output_path(input, suffix, ext).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(&ffmpeg);
    cmd.args(build_args(input, &output, preset_args))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|e| format!("ffmpeg başlatılamadı: {e}"))?;

    let empty = std::fs::metadata(&output).map_or(true, |m| m.len() == 0);
    if !out.status.success() || empty {
        let _ = std::fs::remove_file(&output);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim().lines().last().unwrap_or("bilinmeyen hata");
        return Err(format!("ffmpeg hata verdi: {msg}"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_put_preset_options_between_input_and_output() {
        let a = build_args(
            Path::new("a b.mp4"),
            Path::new("a b-mp3.mp3"),
            &["-vn".into(), "-q:a".into(), "2".into()],
        );
        let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert_eq!(&a[i + 1..], ["a b.mp4", "-vn", "-q:a", "2", "a b-mp3.mp3"]);
        assert!(a.contains(&"-y".to_string()) && a.contains(&"-nostdin".to_string()));
    }
}
