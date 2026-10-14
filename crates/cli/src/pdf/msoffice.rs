//! Microsoft Office ile PDF'e dönüştürme (Word, Excel, PowerPoint'in kendi "PDF olarak dışa
//! aktar" özelliği). Office kuruluysa en yüksek doğruluk budur; COM otomasyonu Windows'ta hazır
//! gelen PowerShell üzerinden yapılır, ek bağımlılık gerekmez.
//!
//! Güvenlik ve kullanıcıyı rahatsız etmeme kuralları:
//! - Belge salt okunur açılır, değişiklik kaydedilmez; makrolar zorla kapatılır
//!   (`AutomationSecurity = 3`), uyarı pencereleri gösterilmez.
//! - Parola korumalı belgeye uydurma bir parola verilir: Office parola sormak yerine hata döner.
//! - Kullanıcının zaten açık olan Office penceresine dokunulmaz: yalnızca bizim başlattığımız
//!   uygulama kapatılır, o da içinde başka belge açık değilse.
//! - Office işleri sırayla yapılır (sistem genelinde kilit); takılan iş süre sınırında
//!   durdurulur ve yalnızca bizim başlattığımız Office süreci sonlandırılır.

use std::path::Path;

pub const HINT: &str = "Bu belge türünü açacak Microsoft Office uygulaması bulunamadı \
     (presets.toml içinde `pdf_engine = \"auto\"` ile LibreOffice ya da yerleşik dönüştürücü kullanılabilir).";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Word,
    Excel,
    PowerPoint,
}

impl App {
    pub fn for_ext(ext: &str) -> Option<App> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" | "doc" | "rtf" | "odt" => Some(App::Word),
            "xlsx" | "xls" | "ods" => Some(App::Excel),
            "pptx" | "ppt" | "odp" => Some(App::PowerPoint),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            App::Word => "Word",
            App::Excel => "Excel",
            App::PowerPoint => "PowerPoint",
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn prog_id(self) -> &'static str {
        match self {
            App::Word => "Word.Application",
            App::Excel => "Excel.Application",
            App::PowerPoint => "PowerPoint.Application",
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn process_name(self) -> &'static str {
        match self {
            App::Word => "WINWORD",
            App::Excel => "EXCEL",
            App::PowerPoint => "POWERPNT",
        }
    }

    /// Uygulama COM için kayıtlı mı (kurulu mu). Etkinleştirilmemiş Office da kayıtlıdır;
    /// o durumda dönüşüm hata verir ve sıradaki motora geçilir.
    pub fn installed(self) -> bool {
        #[cfg(windows)]
        {
            use winreg::RegKey;
            use winreg::enums::HKEY_CLASSES_ROOT;
            RegKey::predef(HKEY_CLASSES_ROOT)
                .open_subkey(format!(r"{}\CLSID", self.prog_id()))
                .is_ok()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

/// PowerShell betiği. Girdiler ortam değişkenleriyle gelir (yollardaki tırnak, `$` vb. sorun
/// olmasın). Betik ASCII'dir: Windows PowerShell 5.1 BOM'suz dosyayı ANSI okur.
#[cfg_attr(not(windows), allow(dead_code))]
const SCRIPT: &str = include_str!("msoffice.ps1");

/// `input`'u ilgili Office uygulamasıyla PDF'e çevirip `out_pdf`'e yazar.
#[cfg(windows)]
pub fn to_pdf(app: App, input: &Path, out_pdf: &Path) -> Result<(), String> {
    use std::process::Command;
    use std::time::Duration;

    use super::process::{self, TempDir};

    const TIMEOUT: Duration = Duration::from_secs(180);

    let _lock = lock::OfficeLock::acquire();
    let work = TempDir::new("msoffice").map_err(|e| e.to_string())?;
    let script = work.0.join("convert.ps1");
    let produced = work.0.join("out.pdf");
    let err_file = work.0.join("error.txt");
    let pid_file = work.0.join("pids.txt");
    std::fs::write(&script, SCRIPT).map_err(|e| e.to_string())?;

    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(&script)
    .env("SC_IN", input)
    .env("SC_OUT", &produced)
    .env("SC_APP", app.name())
    .env("SC_PROGID", app.prog_id())
    .env("SC_PROC", app.process_name())
    .env("SC_ERR", &err_file)
    .env("SC_PIDS", &pid_file);
    let finished = process::run(&mut cmd, TIMEOUT);
    if finished.is_err() {
        // Takılan Office (görünmez bir iletişim kutusu bekliyor olabilir) yalnızca biz
        // başlattıysak sonlandırılır.
        kill_started(&pid_file);
    }
    let finished = finished.map_err(|e| format!("{}: {e}", app.name()))?;
    if !finished || !produced.is_file() {
        let reason = std::fs::read_to_string(&err_file)
            .map(|s| s.trim().trim_start_matches('\u{feff}').to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "PDF üretmedi".into());
        return Err(format!(
            "{}: {reason} (belge bozuk, parola korumalı ya da Office etkinleştirilmemiş olabilir)",
            app.name()
        ));
    }
    std::fs::copy(&produced, out_pdf)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn to_pdf(app: App, _input: &Path, _out_pdf: &Path) -> Result<(), String> {
    Err(format!("{} yalnızca Windows'ta kullanılabilir", app.name()))
}

#[cfg(windows)]
fn kill_started(pid_file: &Path) {
    use std::process::Command;
    let Ok(pids) = std::fs::read_to_string(pid_file) else {
        return;
    };
    for pid in pids
        .split(',')
        .map(str::trim)
        .filter(|p| p.parse::<u32>().is_ok())
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/F", "/PID", pid]);
        let _ = super::process::run(&mut cmd, std::time::Duration::from_secs(10));
    }
}

/// Office işlerini sistem genelinde sıraya koyan adlandırılmış mutex. Aynı anda iki Word
/// otomasyonu birbirinin uygulamasını kapatabilir ya da süreç ayırt etmeyi bozabilir.
#[cfg(windows)]
mod lock {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Threading::{
        CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject,
    };

    pub struct OfficeLock(HANDLE);

    impl OfficeLock {
        /// Kilit alınamazsa (beklenmez) kilitsiz devam edilir.
        pub fn acquire() -> Option<OfficeLock> {
            let name: Vec<u16> = "Local\\ShockConvertOffice\0".encode_utf16().collect();
            // SAFETY: ad NUL ile biten geçerli bir UTF-16 dizisi; tanıtıcı Drop'ta kapatılır.
            let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if h.is_null() {
                return None;
            }
            // WAIT_ABANDONED (çöken bir sahipten kalan) da kilidin bize geçtiği anlamına gelir.
            // SAFETY: h geçerli bir mutex tanıtıcısı.
            unsafe { WaitForSingleObject(h, INFINITE) };
            Some(OfficeLock(h))
        }
    }

    impl Drop for OfficeLock {
        fn drop(&mut self) {
            // SAFETY: tanıtıcı acquire'da açıldı ve sahibi bu iş parçacığı.
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_extensions_to_office_apps() {
        assert_eq!(App::for_ext("DOCX"), Some(App::Word));
        assert_eq!(App::for_ext("rtf"), Some(App::Word));
        assert_eq!(App::for_ext("xls"), Some(App::Excel));
        assert_eq!(App::for_ext("odp"), Some(App::PowerPoint));
        assert_eq!(App::for_ext("txt"), None);
    }

    #[test]
    fn script_is_ascii_for_windows_powershell() {
        assert!(SCRIPT.is_ascii());
    }
}
