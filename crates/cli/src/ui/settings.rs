//! Ayarlar penceresi: genel ayarlar, menü ve kullanıcı preset'leri.

use std::sync::Arc;

use serde_json::{Value, json};

use super::{App, store};
use crate::presets;

struct Settings;

impl App for Settings {
    fn page(&self) -> &'static str {
        include_str!("settings.html")
    }

    fn window_size(&self) -> (u32, u32) {
        (1040, 820)
    }

    fn api(&self, method: &str, path: &str, body: &[u8]) -> Option<(&'static str, Value)> {
        Some(match (method, path) {
            ("GET", "/api/state") => match state() {
                Ok(v) => ("200 OK", v),
                Err(e) => super::error("500 Internal Server Error", e),
            },
            ("POST", "/api/save") => save(body),
            ("POST", "/api/open-file") => match presets::ensure_sample() {
                Ok(path) => {
                    open_in_editor(&path);
                    ("200 OK", json!({ "ok": true }))
                }
                Err(e) => super::error("500 Internal Server Error", e.to_string()),
            },
            _ => return None,
        })
    }
}

/// Ayarlar penceresini açar ve kapanana kadar bekler.
pub fn open(print_url: bool) -> Result<(), String> {
    super::serve(Arc::new(Settings), print_url)
}

/// Menüyü güncel preset'lerle yeniden kaydeder (Windows'ta); hata iletisini döndürür.
pub(crate) fn reregister_menu() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    match std::env::current_exe() {
        Ok(exe) => crate::platform::register(&exe, &crate::load_entries()).err(),
        Err(e) => Some(e.to_string()),
    }
}

/// Arayüzün ilk açılışta ihtiyaç duyduğu her şey.
fn state() -> Result<Value, String> {
    let values = store::Values::current()?;
    let (settings, warnings) = presets::load();
    let builtins: Vec<Value> = presets::defaults()
        .iter()
        .map(|p| json!({ "id": p.id, "label": p.label, "output": p.output }))
        .collect();
    let formats: Vec<Value> = shock_convert_core::Format::ALL
        .iter()
        .map(|f| json!({ "ext": f.ext(), "label": f.label() }))
        .collect();
    Ok(json!({
        "values": values,
        "builtins": builtins,
        "formats": formats,
        "browser": super::locate_browser(settings.browser_path.as_deref())
            .map(|p| p.display().to_string()),
        "config_path": crate::config::presets_path().display().to_string(),
        "warnings": warnings,
        "version": env!("CARGO_PKG_VERSION"),
        "windows": cfg!(windows),
    }))
}

fn save(body: &[u8]) -> (&'static str, Value) {
    let values: store::Values = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return (
                "400 Bad Request",
                json!({ "errors": [["", e.to_string()]] }),
            );
        }
    };
    if let Err(errors) = store::save(&values) {
        return ("400 Bad Request", json!({ "errors": errors }));
    }
    // Menü yeni preset'lerle yeniden kaydedilir (Windows'ta).
    let menu = reregister_menu();
    let (_, warnings) = presets::load();
    (
        "200 OK",
        json!({ "ok": true, "menu_error": menu, "warnings": warnings }),
    )
}

fn open_in_editor(path: &std::path::Path) {
    let mut cmd = if cfg!(windows) {
        std::process::Command::new("notepad.exe")
    } else {
        std::process::Command::new("xdg-open")
    };
    let _ = cmd.arg(path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_rejects_malformed_body() {
        let (status, v) = Settings.api("POST", "/api/save", b"{}").unwrap();
        assert_eq!(status, "400 Bad Request");
        assert!(v["errors"].is_array());
        assert!(Settings.api("GET", "/api/yok", b"").is_none());
    }
}
