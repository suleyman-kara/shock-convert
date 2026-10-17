//! Pencereler (ayarlar, özel dönüştürme). Ayrı bir arayüz kitaplığı yerine Windows'ta zaten
//! bulunan Edge "uygulama penceresi" (`--app`, sekmesiz ve adres çubuksuz) kullanılır; sayfayı bu
//! süreç yalnızca `127.0.0.1` üzerinde, rastgele bir portta ve rastgele bir anahtarla sunar.
//!
//! Güvenlik: sayfa adresi anahtarı taşır; her API çağrısı `X-Token` başlığıyla gelmek zorundadır
//! (başka bir sitenin tarayıcıdan bu başlıkla istek atması CORS ön kontrolüne takılır, biz de
//! ön kontrole izin vermeyiz). `Host` başlığı denetlenir (DNS yeniden bağlama saldırısına karşı).
//! Pencere kapanınca (sayfanın düzenli "buradayım" çağrıları kesilince) sunucu kendiliğinden
//! kapanır; süren bir iş varsa iş bitene kadar bekler.

pub mod custom;
pub mod settings;
mod store;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// Sayfa açılmazsa (tarayıcı başlamadı) bu süre sonunda kapan.
const LOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Sayfa her 5 sn'de bir haber verir; bu kadar sessizlik pencerenin kapandığı anlamına gelir.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY: usize = 2 * 1024 * 1024;

/// Bir pencerenin sayfası ve API'si.
pub trait App: Send + Sync + 'static {
    /// Gömülü HTML.
    fn page(&self) -> &'static str;
    /// Pencere boyutu (piksel).
    fn window_size(&self) -> (u32, u32);
    /// `/api/...` isteğini karşılar; bilinmeyen yol için `None` (404).
    fn api(&self, method: &str, path: &str, body: &[u8]) -> Option<(&'static str, Value)>;
    /// Süren bir iş var mı (varsa pencere kapansa da sunucu beklemeye devam eder).
    fn busy(&self) -> bool {
        false
    }
    /// Pencere kapandıktan ve iş bittikten sonra, çıkmadan hemen önce çağrılır.
    fn on_exit(&self, _window_open: bool) {}
}

struct Server {
    token: String,
    host: String,
    last_seen: Mutex<Option<Instant>>,
    closing: AtomicBool,
    app: Arc<dyn App>,
}

/// Pencereyi açar ve kapanana (ve süren iş bitene) kadar bekler. `print_url`: tarayıcı
/// başlatmadan adresi yazar (test ve sorun giderme için).
pub fn serve(app: Arc<dyn App>, print_url: bool) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let server = Arc::new(Server {
        token: random_token(),
        host: format!("127.0.0.1:{port}"),
        last_seen: Mutex::new(None),
        closing: AtomicBool::new(false),
        app,
    });
    let url = format!("http://{}/?t={}", server.host, server.token);
    if print_url {
        println!("{url}");
    } else {
        launch_window(&url, server.app.window_size())?;
    }

    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // Windows'ta kabul edilen bağlantı dinleyicinin engellemesiz kipini devralır;
                // istek henüz gelmemişse okuma "WouldBlock" ile düşer ve bağlantı yanıtsız kapanırdı.
                if let Err(e) = stream.set_nonblocking(false) {
                    crate::log::line(&format!("window: {e}"));
                    continue;
                }
                let s = Arc::clone(&server);
                std::thread::spawn(move || {
                    if let Err(e) = handle(&s, stream) {
                        crate::log::line(&format!("window: {e}"));
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
        let seen = *server.last_seen.lock().unwrap();
        let gone = match seen {
            _ if server.closing.load(Ordering::Relaxed) => true,
            None => started.elapsed() > LOAD_TIMEOUT,
            Some(t) => t.elapsed() > IDLE_TIMEOUT,
        };
        if gone && !server.app.busy() {
            // Son yanıtın gitmesi için kısa bir süre tanı.
            std::thread::sleep(Duration::from_millis(200));
            let window_open = seen.is_some_and(|t| t.elapsed() < IDLE_TIMEOUT)
                && !server.closing.load(Ordering::Relaxed);
            server.app.on_exit(window_open);
            return Ok(());
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

fn launch_window(url: &str, (w, h): (u32, u32)) -> Result<(), String> {
    let (settings, _) = crate::presets::load();
    let browser = locate_browser(settings.browser_path.as_deref()).ok_or(BROWSER_HINT)?;
    // Ayrı profil: kullanıcının tarayıcı oturumuna, sekmelerine ve eklentilerine dokunulmaz.
    let profile = crate::log::path()
        .parent()
        .map(|p| p.join("settings-window"))
        .unwrap_or_else(|| std::env::temp_dir().join("shock-convert-settings-window"));
    let mut cmd = std::process::Command::new(browser);
    cmd.arg(format!("--app={url}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--window-size={w},{h}"))
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
        ]);
    if std::env::var_os("SHOCK_CONVERT_NO_SANDBOX").is_some() {
        cmd.arg("--no-sandbox");
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open window: {e}"))
}

const BROWSER_HINT: &str = "Microsoft Edge or Google Chrome is needed for this window but was not \
     found. You can set its path with `browser_path` in presets.toml.";

/// Pencere için tarayıcı. Sırayla: yapılandırılmış yol, `SHOCK_CONVERT_BROWSER`, Windows'ta
/// Edge/Chrome kurulum yolları, `PATH`. Windows 10/11'de Edge her zaman vardır.
pub(crate) fn locate_browser(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = configured.filter(|p| p.is_file()) {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("SHOCK_CONVERT_BROWSER")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
            candidates.push(base.join(r"Microsoft\Edge\Application\msedge.exe"));
            candidates.push(base.join(r"Google\Chrome\Application\chrome.exe"));
        }
    }
    if let Some(c) = candidates.into_iter().find(|c| c.is_file()) {
        return Some(c);
    }
    let names = [
        "msedge",
        "microsoft-edge",
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ];
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        names.iter().find_map(|n| {
            let exe = dir.join(if cfg!(windows) {
                format!("{n}.exe")
            } else {
                (*n).to_string()
            });
            exe.is_file().then_some(exe)
        })
    })
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
            return Err("too many headers".into());
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
        return Err("request body too large".into());
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
            let page = server.app.page();
            respond(
                &stream,
                "200 OK",
                "text/html; charset=utf-8",
                page.as_bytes(),
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
        ("POST", "/api/close") => {
            respond_json(&stream, "200 OK", &json!({ "ok": true }));
            server.closing.store(true, Ordering::Relaxed);
        }
        (method, path) => match server.app.api(method, path, &req.body) {
            Some((status, v)) => respond_json(&stream, status, &v),
            None => respond(&stream, "404 Not Found", "text/plain", b"not found"),
        },
    }
    Ok(())
}

/// JSON hata yanıtı.
pub(crate) fn error(status: &'static str, message: impl Into<String>) -> (&'static str, Value) {
    (status, json!({ "error": message.into() }))
}

/// Dosyayı Gezgin'de seçili gösterir (Windows) ya da klasörünü açar.
pub(crate) fn reveal(path: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut arg = std::ffi::OsString::from("/select,\"");
        arg.push(path.as_os_str());
        arg.push("\"");
        let _ = std::process::Command::new("explorer.exe")
            .raw_arg(arg)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(dir) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl App for Echo {
        fn page(&self) -> &'static str {
            "<html>test</html>"
        }
        fn window_size(&self) -> (u32, u32) {
            (100, 100)
        }
        fn api(&self, method: &str, path: &str, _: &[u8]) -> Option<(&'static str, Value)> {
            (method == "GET" && path == "/api/state").then(|| ("200 OK", json!({ "ok": 1 })))
        }
    }

    /// Gerçek sunucuyu bir iş parçacığında başlatıp HTTP ile konuşur.
    fn start(requests: usize) -> (String, String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(Server {
            token: random_token(),
            host: format!("127.0.0.1:{port}"),
            last_seen: Mutex::new(None),
            closing: AtomicBool::new(false),
            app: Arc::new(Echo),
        });
        let (host, token) = (server.host.clone(), server.token.clone());
        let h = std::thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
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
        let (host, token, h) = start(7);
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

        let state = call(
            &host,
            &format!("GET /api/state HTTP/1.1\r\nHost: {host}\r\nX-Token: {token}\r\n\r\n"),
        );
        assert!(
            state.starts_with("HTTP/1.1 200") && state.contains("\"ok\":1"),
            "{state}"
        );

        let ping = call(
            &host,
            &format!(
                "POST /api/ping HTTP/1.1\r\nHost: {host}\r\nX-Token: {token}\r\nContent-Length: 0\r\n\r\n"
            ),
        );
        assert!(ping.starts_with("HTTP/1.1 200"), "{ping}");

        let unknown = call(
            &host,
            &format!("GET /api/nope HTTP/1.1\r\nHost: {host}\r\nX-Token: {token}\r\n\r\n"),
        );
        assert!(unknown.starts_with("HTTP/1.1 404"));
        h.join().unwrap();
    }

    #[test]
    fn tokens_are_long_and_unique() {
        let a = random_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, random_token());
    }
}
