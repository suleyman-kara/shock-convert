// Release'te konsol penceresi açılmasın: sağ tıkla başlatınca kullanıcı siyah pencere görmez.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod batch;
mod config;
mod ffmpeg;
mod log;
mod menu;
mod platform;
mod plugins;
mod presets;
mod run;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "shock-convert",
    version,
    about = "Sağ tıkla görüntü, ses ve video dönüştürücü"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dosyaları bir menü girdisiyle işler (örn. `run png a.jpg b.webp`)
    Run {
        /// Menü girdisi kimliği (`list` ile görülür)
        id: String,
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
    },
    /// Sağ tık menüsünün kullandığı komut: tek dosya alır, eşzamanlı çağrıları birleştirir
    #[command(hide = true)]
    ShellRun { id: String, file: PathBuf },
    /// Kullanılabilir menü girdilerini listeler
    List,
    /// Preset dosyasının (presets.toml) yolunu yazar; yoksa açıklamalı örneği oluşturur
    Presets,
    /// Sağ tık menüsünü kayıt defterine yazar (eklentileri yeniden tarar)
    Register,
    /// Sağ tık menüsünü kayıt defterinden siler
    Unregister,
    /// Kendini kalıcı konuma kurar ve menüyü kaydeder
    Install,
    /// Menüyü ve kurulumu tamamen kaldırır
    Uninstall,
}

fn plugins_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("plugins")))
        .unwrap_or_else(|| PathBuf::from("plugins"))
}

/// Preset'ler + `plugins_dir` altındaki eklentilerden menü girdilerini kurar.
pub(crate) fn entries_for(plugins_dir: &std::path::Path) -> Vec<menu::Entry> {
    let (settings, mut warnings) = presets::load();
    let (plugins, plugin_warnings) = plugins::load_all(plugins_dir);
    warnings.extend(plugin_warnings);
    for w in warnings {
        log::line(&format!("uyarı: {w}"));
        eprintln!("uyarı: {w}");
    }
    menu::build(&settings, &plugins)
}

fn load_entries() -> Vec<menu::Entry> {
    entries_for(&plugins_dir())
}

fn main() {
    platform::init_console();
    log::line(&format!(
        "başladı: {:?}",
        std::env::args().collect::<Vec<_>>()
    ));
    std::panic::set_hook(Box::new(|info| log::line(&format!("PANIC: {info}"))));

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Sağ tıkla başlatıldığında konsol olmadığı için hatayı günlüğe ve bildirime de yaz.
            let text = e.to_string();
            log::line(&format!("argüman hatası: {text}"));
            if e.use_stderr() {
                platform::notify(
                    "Shock Convert",
                    text.lines().next().unwrap_or("argüman hatası"),
                );
            }
            e.exit()
        }
    };

    let code = match cli.command {
        Command::Run { id, files } => run(&id, &files),
        Command::ShellRun { id, file } => shell_run(&id, &file),
        Command::List => {
            for e in load_entries() {
                println!("{:<24} {}", e.id, e.label);
            }
            0
        }
        Command::Presets => finish_with(presets::ensure_sample().map(|p| p.display().to_string())),
        Command::Register => finish("Menü kaydedildi.", register()),
        Command::Unregister => finish("Menü kaldırıldı.", platform::unregister()),
        Command::Install => finish("Shock Convert kuruldu.", platform::install()),
        Command::Uninstall => finish("Shock Convert kaldırıldı.", platform::uninstall()),
    };
    std::process::exit(code);
}

fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // İlk kayıtta açıklamalı örnek presets.toml bırak (varsa dokunma).
    if let Err(e) = presets::ensure_sample() {
        log::line(&format!("presets.toml örneği yazılamadı: {e}"));
    }
    platform::register(&exe, &load_entries())
}

fn finish_with(result: std::io::Result<String>) -> i32 {
    match result {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(e) => {
            eprintln!("hata: {e}");
            1
        }
    }
}

fn finish(ok_message: &str, result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => {
            println!("{ok_message}");
            0
        }
        Err(e) => {
            eprintln!("hata: {e}");
            1
        }
    }
}

/// Explorer çoklu seçimde her dosya için ayrı süreç başlatır; burada tek işe birleştirilir.
fn shell_run(id: &str, file: &std::path::Path) -> i32 {
    let mut code = 0;
    let result = batch::submit(id, file, std::time::Duration::from_millis(400), |files| {
        code |= run(id, &files);
    });
    if let Err(e) = result {
        log::line(&format!("kuyruk hatası: {e}"));
        // Kuyruk kullanılamadıysa en azından bu dosyayı doğrudan işle.
        return run(id, &[file.to_path_buf()]);
    }
    code
}

fn run(id: &str, files: &[PathBuf]) -> i32 {
    log::line(&format!("run id={id} dosya={files:?}"));
    let entries = load_entries();
    let Some(entry) = entries.iter().find(|e| e.id.eq_ignore_ascii_case(id)) else {
        log::line(&format!("bilinmeyen menü girdisi: {id}"));
        platform::notify("Shock Convert", &format!("Bilinmeyen menü girdisi: {id}"));
        return 2;
    };
    if matches!(entry.action, menu::Action::Ffmpeg(_)) {
        // Ses/video uzun sürebilir; başladığını hemen göster.
        platform::notify(
            "Shock Convert",
            &format!("{} dosya → {} dönüştürülüyor…", files.len(), entry.label),
        );
    }
    let report = run::run_entry(entry, files);
    let summary = report.summary(entry);
    log::line(&format!("sonuç: {summary}"));
    if !summary.is_empty() {
        platform::notify("Shock Convert", &summary);
        println!("{summary}");
    }
    i32::from(!report.failed.is_empty())
}
