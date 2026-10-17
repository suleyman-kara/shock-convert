// Release'te konsol penceresi açılmasın: sağ tıkla başlatınca kullanıcı siyah pencere görmez.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod batch;
mod config;
mod job;
mod log;
mod menu;
mod platform;
mod plugins;
mod presets;
mod run;
mod ui;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "shock-convert",
    version,
    about = "Right-click image converter"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Converts files with a menu entry (e.g. `run png a.jpg b.webp`)
    Run {
        /// Menu entry ID (see `list`)
        id: String,
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
    },
    /// Used by the right-click menu: takes one file and merges concurrent calls
    #[command(hide = true)]
    ShellRun { id: String, file: PathBuf },
    /// Lists the available menu entries
    List,
    /// Prints the path of presets.toml, creating a commented sample if missing
    Presets,
    /// Writes the right-click menu to the registry (rescans plugins)
    Register,
    /// Removes the right-click menu from the registry
    Unregister,
    /// Installs itself to a permanent location and registers the menu
    Install,
    /// Removes the menu and the installation
    Uninstall,
    /// Opens the custom convert window for the given files (one-off settings)
    Custom {
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
        /// Prints the address instead of opening a window (troubleshooting)
        #[arg(long, hide = true)]
        print_url: bool,
    },
    /// Used by the menu's "Custom convert…" item: gathers a multi-selection into one window
    #[command(hide = true)]
    ShellCustom { file: PathBuf },
    /// Opens the settings window
    Settings {
        /// Prints the address instead of opening a window (troubleshooting)
        #[arg(long, hide = true)]
        print_url: bool,
    },
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
        log::line(&format!("warning: {w}"));
        eprintln!("warning: {w}");
    }
    menu::build(&settings, &plugins)
}

pub(crate) fn load_entries() -> Vec<menu::Entry> {
    entries_for(&plugins_dir())
}

fn main() {
    platform::init_console();
    log::line(&format!(
        "started: {:?}",
        std::env::args().collect::<Vec<_>>()
    ));
    std::panic::set_hook(Box::new(|info| log::line(&format!("PANIC: {info}"))));

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Sağ tıkla başlatıldığında konsol olmadığı için hatayı günlüğe ve bildirime de yaz.
            let text = e.to_string();
            log::line(&format!("argument error: {text}"));
            if e.use_stderr() {
                platform::notify(
                    "Shock Convert",
                    text.lines().next().unwrap_or("argument error"),
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
        Command::Register => finish("Menu registered.", register()),
        Command::Unregister => finish("Menu removed.", platform::unregister()),
        Command::Install => finish("Shock Convert installed.", platform::install()),
        Command::Uninstall => finish("Shock Convert uninstalled.", platform::uninstall()),
        Command::Custom { files, print_url } => window(ui::custom::open(files, print_url)),
        Command::ShellCustom { file } => shell_custom(&file),
        Command::Settings { print_url } => match ui::settings::open(print_url) {
            Ok(()) => 0,
            Err(e) => {
                log::line(&format!("settings: {e}"));
                platform::notify("Shock Convert", &e);
                1
            }
        },
    };
    std::process::exit(code);
}

fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // İlk kayıtta açıklamalı örnek presets.toml bırak (varsa dokunma).
    if let Err(e) = presets::ensure_sample() {
        log::line(&format!("could not write sample presets.toml: {e}"));
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
            eprintln!("error: {e}");
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
            eprintln!("error: {e}");
            1
        }
    }
}

fn window(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            log::line(&format!("window: {e}"));
            platform::notify("Shock Convert", &e);
            1
        }
    }
}

/// Çoklu seçimdeki dosyalar kuyrukta toplanır; pencere kuyruk serbest kaldıktan sonra açılır,
/// böylece pencere açıkken yapılan yeni bir sağ tık kendi penceresini açabilir.
fn shell_custom(file: &std::path::Path) -> i32 {
    let mut files = Vec::new();
    let collected = batch::submit(
        "custom",
        file,
        std::time::Duration::from_millis(400),
        |batch| {
            files.extend(batch);
        },
    );
    if let Err(e) = collected {
        log::line(&format!("queue error: {e}"));
        files = vec![file.to_path_buf()];
    }
    if files.is_empty() {
        return 0; // Takipçi: dosyamızı lider aldı.
    }
    window(ui::custom::open(files, false))
}

/// Explorer çoklu seçimde her dosya için ayrı süreç başlatır; burada tek işe birleştirilir.
fn shell_run(id: &str, file: &std::path::Path) -> i32 {
    let mut code = 0;
    let result = batch::submit(id, file, std::time::Duration::from_millis(400), |files| {
        code |= run(id, &files);
    });
    if let Err(e) = result {
        log::line(&format!("queue error: {e}"));
        // Kuyruk kullanılamadıysa en azından bu dosyayı doğrudan işle.
        return run(id, &[file.to_path_buf()]);
    }
    code
}

fn run(id: &str, files: &[PathBuf]) -> i32 {
    log::line(&format!("run id={id} files={files:?}"));
    let entries = load_entries();
    let Some(entry) = entries.iter().find(|e| e.id.eq_ignore_ascii_case(id)) else {
        log::line(&format!("unknown menu entry: {id}"));
        platform::notify("Shock Convert", &format!("Unknown menu entry: {id}"));
        return 2;
    };
    let report = run::run_entry(entry, files);
    let summary = report.summary(entry);
    log::line(&format!("result: {summary}"));
    if !summary.is_empty() {
        platform::notify("Shock Convert", &summary);
        println!("{summary}");
    }
    i32::from(!report.failed.is_empty())
}
