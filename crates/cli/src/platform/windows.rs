use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::IntoRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    SetStdHandle,
};
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

use crate::menu::Entry;

const ASSOC_ROOT: &str = r"Software\Classes\SystemFileAssociations";
const MENU_KEY: &str = "ShockConvert";
const MENU_TITLE: &str = "Shock Convert";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert";

/// GUI alt sisteminde derlendiği için terminalden çağrılınca çıktının görünmesini sağlar.
pub fn init_console() {
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return; // Explorer'dan başlatıldı, konsol yok.
        }
        for (std_id, name) in [
            (STD_OUTPUT_HANDLE, "CONOUT$"),
            (STD_ERROR_HANDLE, "CONOUT$"),
        ] {
            let h = GetStdHandle(std_id);
            if (h.is_null() || h == INVALID_HANDLE_VALUE)
                && let Ok(f) = std::fs::OpenOptions::new().write(true).open(name)
            {
                SetStdHandle(std_id, f.into_raw_handle());
            }
        }
    }
}

pub fn notify(title: &str, text: &str) {
    use winrt_notification::Toast;
    let _ = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(title)
        .text1(text)
        .show();
}

fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

fn refresh_shell() {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
}

/// Her dosya uzantısı için `Shock Convert ▸` alt menüsünü kaydeder (klasik sağ tık menüsü).
pub fn register(exe: &Path, entries: &[Entry]) -> Result<(), String> {
    unregister()?;
    let exe = exe.display();

    let mut exts: Vec<&str> = entries
        .iter()
        .flat_map(|e| e.input_exts.iter().map(String::as_str))
        .collect();
    exts.sort_unstable();
    exts.dedup();

    for ext in exts {
        let base = format!(r"{ASSOC_ROOT}\.{ext}\shell\{MENU_KEY}");
        let (parent, _) = hkcu().create_subkey(&base).map_err(|e| e.to_string())?;
        parent
            .set_value("MUIVerb", &MENU_TITLE)
            .map_err(|e| e.to_string())?;
        parent
            .set_value("Icon", &format!("\"{exe}\",0"))
            .map_err(|e| e.to_string())?;
        parent
            .set_value("SubCommands", &"")
            .map_err(|e| e.to_string())?;
        // Seçili tüm dosyalar tek süreçte işlensin (tek bildirim).
        parent
            .set_value("MultiSelectModel", &"Player")
            .map_err(|e| e.to_string())?;

        for (idx, entry) in entries.iter().enumerate().filter(|(_, e)| e.accepts(ext)) {
            let key_name = format!("{idx:02}-{}", entry.id.replace([':', '\\', '/'], "_"));
            let (item, _) = hkcu()
                .create_subkey(format!(r"{base}\shell\{key_name}"))
                .map_err(|e| e.to_string())?;
            item.set_value("MUIVerb", &entry.label)
                .map_err(|e| e.to_string())?;
            item.set_value("MultiSelectModel", &"Player")
                .map_err(|e| e.to_string())?;
            let (cmd, _) = item.create_subkey("command").map_err(|e| e.to_string())?;
            cmd.set_value("", &format!("\"{exe}\" run \"{}\" %*", entry.id))
                .map_err(|e| e.to_string())?;
        }
    }
    refresh_shell();
    Ok(())
}

/// Kayıt defterinde bu uygulamanın eklediği tüm menü anahtarlarını siler,
/// geride kalan boş `shell` / uzantı anahtarlarını da temizler.
pub fn unregister() -> Result<(), String> {
    let Ok(root) = hkcu().open_subkey_with_flags(ASSOC_ROOT, KEY_READ) else {
        return Ok(());
    };
    let names: Vec<String> = root
        .enum_keys()
        .filter_map(Result::ok)
        .filter(|n| n.starts_with('.'))
        .collect();

    for name in names {
        let shell_path = format!(r"{ASSOC_ROOT}\{name}\shell");
        let menu_path = format!(r"{shell_path}\{MENU_KEY}");
        if hkcu().open_subkey(&menu_path).is_err() {
            continue;
        }
        hkcu()
            .delete_subkey_all(&menu_path)
            .map_err(|e| e.to_string())?;
        for path in [shell_path, format!(r"{ASSOC_ROOT}\{name}")] {
            if is_empty_key(&path) {
                let _ = hkcu().delete_subkey(&path);
            }
        }
    }
    refresh_shell();
    Ok(())
}

fn is_empty_key(path: &str) -> bool {
    hkcu()
        .open_subkey(path)
        .map(|k| k.enum_keys().next().is_none() && k.enum_values().next().is_none())
        .unwrap_or(false)
}

fn install_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA tanımlı değil")?;
    Ok(PathBuf::from(base).join("Programs").join("ShockConvert"))
}

/// Tek başına kurulum: exe'yi (ve varsa yanındaki `plugins` klasörünü) kalıcı konuma
/// kopyalar, menüyü kaydeder, "Uygulamalar ve özellikler" listesine kaldırma girdisi ekler.
pub fn install() -> Result<(), String> {
    let src = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = install_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("shock-convert.exe");
    if src != dest {
        std::fs::copy(&src, &dest).map_err(|e| format!("exe kopyalanamadı: {e}"))?;
    }
    if let Some(src_plugins) = src
        .parent()
        .map(|p| p.join("plugins"))
        .filter(|p| p.is_dir() && *p != dir.join("plugins"))
    {
        copy_dir(&src_plugins, &dir.join("plugins")).map_err(|e| e.to_string())?;
    }

    let (plugins, _) = crate::plugins::load_all(&dir.join("plugins"));
    register(&dest, &crate::menu::build(&plugins))?;

    let (k, _) = hkcu()
        .create_subkey(UNINSTALL_KEY)
        .map_err(|e| e.to_string())?;
    let exe = dest.display();
    let set = |name: &str, v: &str| k.set_value(name, &v).map_err(|e| e.to_string());
    set("DisplayName", MENU_TITLE)?;
    set("DisplayVersion", env!("CARGO_PKG_VERSION"))?;
    set("Publisher", "Shock Convert")?;
    set("DisplayIcon", &format!("{exe},0"))?;
    set("InstallLocation", &dir.display().to_string())?;
    set("UninstallString", &format!("\"{exe}\" uninstall"))?;
    set("QuietUninstallString", &format!("\"{exe}\" uninstall"))?;
    k.set_value("NoModify", &1u32).map_err(|e| e.to_string())?;
    k.set_value("NoRepair", &1u32).map_err(|e| e.to_string())?;
    Ok(())
}

/// Menüyü, kaldırma girdisini ve kurulum klasörünü siler. Çalışan exe kendini
/// silemeyeceği için klasör, süreç bittikten sonra ayrı bir `cmd` ile silinir.
pub fn uninstall() -> Result<(), String> {
    unregister()?;
    let _ = hkcu().delete_subkey_all(UNINSTALL_KEY);
    let dir = install_dir()?;
    if dir.exists() {
        let wide_ok = dir.as_os_str().encode_wide().all(|c| c != u16::from(b'"'));
        if !wide_ok {
            return Err("kurulum yolu geçersiz karakter içeriyor".into());
        }
        Command::new("cmd")
            .raw_arg(format!(
                r#"/C ping 127.0.0.1 -n 3 >nul & rmdir /S /Q "{}""#,
                dir.display()
            ))
            .creation_flags(0x0800_0000 | 0x0000_0008) // CREATE_NO_WINDOW | DETACHED_PROCESS
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
