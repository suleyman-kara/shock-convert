//! Ayarlar penceresi. Ayrı bir arayüz kitaplığı yerine Windows'ta zaten bulunan Edge "uygulama
//! penceresi" (`--app`, sekmesiz ve adres çubuksuz) kullanılır; sayfayı bu süreç yalnızca
//! `127.0.0.1` üzerinde, rastgele bir portta ve rastgele bir anahtarla sunar.
//!
//! Güvenlik: sayfa adresi anahtarı taşır; her API çağrısı `X-Token` başlığıyla gelmek zorundadır
//! (başka bir sitenin tarayıcıdan bu başlıkla istek atması CORS ön kontrolüne takılır, biz de
//! ön kontrole izin vermeyiz). `Host` başlığı denetlenir (DNS yeniden bağlama saldırısına karşı).
//! Pencere kapanınca (sayfanın düzenli "buradayım" çağrıları kesilince) sunucu kendiliğinden kapanır.

mod store;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::presets::{self, Kind};

const PAGE: &str = include_str!("page.html");
/// Sayfa açılmazsa (tarayıcı başlamadı) bu süre sonunda kapan.
const LOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Sayfa her 5 sn'de bir haber verir; bu kadar sessizlik pencerenin kapandığı anlamına gelir.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY: usize = 2 * 1024 * 1024;

struct Server {
    token: String,
    host: String,
    last_seen: Mutex<Option<Instant>>,
    closing: AtomicBool,
}

/// Ayarlar penceresini açar ve kapanana kadar bekler. `print_url`: tarayıcı başlatmadan adresi
/// yazar (test ve sorun giderme için).
pub fn open(print_url: bool) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let server = Arc::new(Server {
        token: random_token(),
        host: format!("127.0.0.1:{port}"),
        last_seen: Mutex::new(None),
        closing: AtomicBool::new(false),
    });
    let url = format!("http://{}/?t={}", server.host, server.token);
    if print_url {
        println!("{url}");
    } else {
        launch_window(&url)?;
    }

    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let s = Arc::clone(&server);
                std::thread::spawn(move || {
                    if let Err(e) = handle(&s, stream) {
                        crate::log::line(&format!("ayarlar: {e}"));
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
        if server.closing.load(Ordering::Relaxed) {
            // Son yanıtın gitmesi için kısa bir süre tanı.
            std::thread::sleep(Duration::from_millis(200));
            return Ok(());
        }
        let seen = *server.last_seen.lock().unwrap();
        match seen {
            None if started.elapsed() > LOAD_TIMEOUT => return Ok(()),
            Some(t) if t.elapsed() > IDLE_TIMEOUT => return Ok(()),
            _ => {}
        }
    }
}

fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    (0..4u64)
        .map(|i| {
            // RandomState anahtarları işletim sisteminin rastgele kaynağından gelir.
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(nanos);
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}

fn launch_window(url: &str) -> Result<(), String> {
    let (settings, _) = presets::load();
    let browser = crate::pdf::browser::locate(settings.browser_path.as_deref())
        .ok_or_else(|| crate::pdf::browser::HINT.to_string())?;
    // Ayrı profil: kullanıcının tarayıcı oturumuna, sekmelerine ve eklentilerine dokunulmaz.
    let profile = crate::log::path()
        .parent()
        .map(|p| p.join("settings-window"))
        .unwrap_or_else(|| std::env::temp_dir().join("shock-convert-settings-window"));
    let mut cmd = std::process::Command::new(browser);
    cmd.arg(format!("--app={url}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--window-size=1040,820",
        ]);
    if std::env::var_os("SHOCK_CONVERT_NO_SANDBOX").is_some() {
        cmd.arg("--no-sandbox");
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("ayarlar penceresi açılamadı: {e}"))
}

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn read_request(stream: &TcpStream) -> Result<Request, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.take(MAX_BODY as u64 + 64 * 1024));
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (path.to_string(), query.to_string());

    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if headers.len() > 100 {
            return Err("çok fazla başlık".into());
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let len: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    if len > MAX_BODY {
        return Err("istek gövdesi çok büyük".into());
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn respond(mut stream: &TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline'; connect-src 'self'; img-src data:; base-uri 'none'; \
         form-action 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn respond_json(stream: &TcpStream, status: &str, v: &Value) {
    respond(
        stream,
        status,
        "application/json; charset=utf-8",
        v.to_string().as_bytes(),
    );
}

fn handle(server: &Server, stream: TcpStream) -> Result<(), String> {
    let req = read_request(&stream)?;
    if req.header("host") != Some(server.host.as_str()) {
        respond(&stream, "403 Forbidden", "text/plain", b"forbidden");
        return Ok(());
    }
    if req.method == "GET" && req.path == "/" {
        let token_ok = req
            .query
            .split('&')
            .any(|kv| kv == format!("t={}", server.token));
        if token_ok {
            respond(
                &stream,
                "200 OK",
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
            );
        } else {
            respond(&stream, "403 Forbidden", "text/plain", b"forbidden");
        }
        return Ok(());
    }
    if !req.path.starts_with("/api/") || req.header("x-token") != Some(server.token.as_str()) {
        respond(&stream, "403 Forbidden", "text/plain", b"forbidden");
        return Ok(());
    }
    *server.last_seen.lock().unwrap() = Some(Instant::now());
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/api/ping") => respond_json(&stream, "200 OK", &json!({ "ok": true })),
        ("GET", "/api/state") => match state() {
            Ok(v) => respond_json(&stream, "200 OK", &v),
            Err(e) => respond_json(&stream, "500 Internal Server Error", &json!({ "error": e })),
        },
        ("POST", "/api/save") => {
            let (status, v) = save(&req.body);
            respond_json(&stream, status, &v);
        }
        ("POST", "/api/open-file") => {
            let path = presets::ensure_sample().map_err(|e| e.to_string())?;
            open_in_editor(&path);
            respond_json(&stream, "200 OK", &json!({ "ok": true }));
        }
        ("POST", "/api/close") => {
            respond_json(&stream, "200 OK", &json!({ "ok": true }));
            server.closing.store(true, Ordering::Relaxed);
        }
        _ => respond(&stream, "404 Not Found", "text/plain", b"not found"),
    }
    Ok(())
}

fn group_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Image => "image",
        Kind::Pdf | Kind::PdfTool => "pdf",
        Kind::Ffmpeg => "media",
    }
}

fn path_or_null(p: Option<PathBuf>) -> Value {
    p.map_or(Value::Null, |p| Value::String(p.display().to_string()))
}

/// Arayüzün ilk açılışta ihtiyaç duyduğu her şey.
fn state() -> Result<Value, String> {
    let values = store::Values::current()?;
    let (settings, warnings) = presets::load();
    let builtins: Vec<Value> = presets::defaults()
        .iter()
        .map(|p| {
            json!({
                "id": p.id, "label": p.label, "kind": p.kind, "output": p.output,
                "group": group_of(p.kind),
            })
        })
        .collect();
    use crate::pdf::msoffice::App;
    let office: Vec<&str> = [App::Word, App::Excel, App::PowerPoint]
        .into_iter()
        .filter(|a| a.installed())
        .map(App::name)
        .collect();
    let formats: Vec<Value> = shock_convert_core::Format::ALL
        .iter()
        .map(|f| json!({ "ext": f.ext(), "label": f.label() }))
        .collect();
    Ok(json!({
        "values": values,
        "builtins": builtins,
        "formats": formats,
        "detected": {
            "ffmpeg": path_or_null(crate::ffmpeg::locate(settings.ffmpeg_path.as_deref())),
            "browser": path_or_null(crate::pdf::browser::locate(settings.browser_path.as_deref())),
            "libreoffice": path_or_null(crate::pdf::libreoffice::locate(settings.libreoffice_path.as_deref())),
            "office": office,
        },
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
    let menu = if cfg!(windows) {
        match std::env::current_exe() {
            Ok(exe) => crate::platform::register(&exe, &crate::load_entries()).err(),
            Err(e) => Some(e.to_string()),
        }
    } else {
        None
    };
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

    /// Gerçek sunucuyu bir iş parçacığında başlatıp HTTP ile konuşur.
    fn start() -> (String, String, std::thread::JoinHandle<()>) {
        // `open` adresi stdout'a yazar; testte port ve anahtarı doğrudan kurmak için
        // sunucuyu aynı mantıkla burada kuruyoruz.
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(Server {
            token: random_token(),
            host: format!("127.0.0.1:{port}"),
            last_seen: Mutex::new(None),
            closing: AtomicBool::new(false),
        });
        let (host, token) = (server.host.clone(), server.token.clone());
        let h = std::thread::spawn(move || {
            for stream in listener.incoming().take(6) {
                handle(&server, stream.unwrap()).unwrap();
            }
        });
        (host, token, h)
    }

    fn call(host: &str, req: &str) -> String {
        let mut s = TcpStream::connect(host).unwrap();
        s.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn page_and_api_require_token_and_local_host() {
        let (host, token, h) = start();
        let page = call(
            &host,
            &format!("GET /?t={token} HTTP/1.1\r\nHost: {host}\r\n\r\n"),
        );
        assert!(
            page.starts_with("HTTP/1.1 200") && page.contains("<html"),
            "{page:.200}"
        );
        assert!(page.contains("Content-Security-Policy"));

        let no_token = call(&host, &format!("GET /? HTTP/1.1\r\nHost: {host}\r\n\r\n"));
        assert!(no_token.starts_with("HTTP/1.1 403"));

        let api_no_token = call(
            &host,
            &format!("GET /api/state HTTP/1.1\r\nHost: {host}\r\n\r\n"),
        );
        assert!(api_no_token.starts_with("HTTP/1.1 403"));

        let rebinding = call(
            &host,
            &format!(
                "GET /api/state HTTP/1.1\r\nHost: evil.example:80\r\nX-Token: {token}\r\n\r\n"
            ),
        );
        assert!(rebinding.starts_with("HTTP/1.1 403"));

        let ping = call(
            &host,
            &format!(
                "POST /api/ping HTTP/1.1\r\nHost: {host}\r\nX-Token: {token}\r\nContent-Length: 0\r\n\r\n"
            ),
        );
        assert!(ping.starts_with("HTTP/1.1 200"), "{ping}");

        let bad_save = call(
            &host,
            &format!(
                "POST /api/save HTTP/1.1\r\nHost: {host}\r\nX-Token: {token}\r\nContent-Length: 2\r\n\r\n{{}}"
            ),
        );
        assert!(bad_save.starts_with("HTTP/1.1 400"), "{bad_save}");
        h.join().unwrap();
    }

    #[test]
    fn tokens_are_long_and_unique() {
        let a = random_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, random_token());
    }
}
