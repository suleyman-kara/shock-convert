//! Kullanıcı ayarlarının konumu. Windows'ta `%APPDATA%\ShockConvert`, diğerlerinde `~/.config/shock-convert`.
//! `SHOCK_CONVERT_CONFIG_DIR` ortam değişkeni (test/taşınabilir kullanım için) bunu ezer.

use std::path::PathBuf;

pub fn dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SHOCK_CONVERT_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        return base.join("ShockConvert");
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("shock-convert")
}

pub fn presets_path() -> PathBuf {
    dir().join("presets.toml")
}
