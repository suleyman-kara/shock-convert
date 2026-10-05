// Release'te konsol penceresi açılmasın: sağ tıkla başlatınca kullanıcı siyah pencere görmez.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod menu;
mod platform;
mod plugins;
mod run;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "shock-convert",
    version,
    about = "Sağ tıkla görüntü dönüştürücü"
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
    /// Kullanılabilir menü girdilerini listeler
    List,
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

fn load_entries() -> Vec<menu::Entry> {
    let (plugins, warnings) = plugins::load_all(&plugins_dir());
    for w in warnings {
        eprintln!("uyarı: eklenti atlandı — {w}");
    }
    menu::build(&plugins)
}

fn main() {
    platform::init_console();
    let cli = Cli::parse();

    let code = match cli.command {
        Command::Run { id, files } => run(&id, &files),
        Command::List => {
            for e in load_entries() {
                println!("{:<24} {}", e.id, e.label);
            }
            0
        }
        Command::Register => finish("Menü kaydedildi.", register()),
        Command::Unregister => finish("Menü kaldırıldı.", platform::unregister()),
        Command::Install => finish("Shock Convert kuruldu.", platform::install()),
        Command::Uninstall => finish("Shock Convert kaldırıldı.", platform::uninstall()),
    };
    std::process::exit(code);
}

fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    platform::register(&exe, &load_entries())
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

fn run(id: &str, files: &[PathBuf]) -> i32 {
    let entries = load_entries();
    let Some(entry) = entries.iter().find(|e| e.id.eq_ignore_ascii_case(id)) else {
        platform::notify("Shock Convert", &format!("Bilinmeyen menü girdisi: {id}"));
        return 2;
    };
    let report = run::run_entry(entry, files);
    let summary = report.summary(entry);
    if !summary.is_empty() {
        platform::notify("Shock Convert", &summary);
        println!("{summary}");
    }
    i32::from(!report.failed.is_empty())
}
