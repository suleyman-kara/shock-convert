# Shock Convert

A small, fast Windows tool that converts images from the **right-click menu** in File Explorer.

**Website and download:** https://shockconvert.suleymankara.com/

```
photo.png  ──right-click──▶  Shock Convert ▸ JPG  ──▶  photo.jpg   (same folder)
```

- Formats: **PNG, JPG, WebP, BMP, GIF, TIFF, ICO** (`jpeg`, `jfif` and `tif` are recognised as input too). Conversion runs in-process; no external programs are started.
- Add your own **presets** in `presets.toml`, e.g. "1920px JPG for the web".
- Select several files at once: they are converted in parallel with a single notification.
- The output is written next to the source **with the same name**: `photo.png` → `photo.jpg`. If the name is taken it becomes `photo (1).jpg`; **existing files are never overwritten**.
- No admin rights needed (everything is installed into your user profile). Shows up in the classic menu on Windows 10 and Windows 11 ("Show more options").
- A single small `.exe`; no Python or other runtime required.

## Install / uninstall

**With winget (recommended):**

```
winget install SuleymanKara.ShockConvert
winget upgrade SuleymanKara.ShockConvert
winget uninstall SuleymanKara.ShockConvert
```

No admin rights are needed; everything is installed into your user profile. Uninstalling removes the menu, registry keys and files completely.

**With the installer:** run `shock-convert-setup-<version>.exe` from [Releases](https://github.com/suleyman-kara/shock-convert/releases). Shock Convert is not code-signed, so Windows SmartScreen may warn when you run a downloaded installer: choose **More info ▸ Run anyway**. The download page shows the installer's SHA-256 checksum to compare against. Installing with winget avoids this warning.

**Without an installer:** `shock-convert.exe install` copies itself to `%LOCALAPPDATA%\Programs\ShockConvert`, registers the menu and adds an uninstall entry. `shock-convert.exe uninstall` undoes all of it.

> The menu points at the exe's location, so don't move the exe after installing; if you do, run `install` again.

## Custom convert

Right-click ▸ Shock Convert ▸ **Custom convert…** (or `shock-convert custom <files>`) opens a window with one-off settings for the selected images: format, quality, max width/height, EXIF and the colour for transparent areas. A multi-selection is gathered into one window.

The window shows progress, and **Cancel** stops after the current file. Results can be opened with "Show in folder". The last settings are remembered; **Save as preset…** adds the same settings to the right-click menu as a permanent entry. If you close the window while a conversion is running, it keeps going and a notification appears when it is done.

## Settings

**Settings…** at the bottom of the right-click menu, or **Shock Convert Settings** in the Start menu (`shock-convert settings`), opens the settings window:

- **General:** keep the source file's dates; photo information (EXIF): keep / remove location / remove all
- **Menu:** which built-in formats appear
- **My presets:** add, edit and delete your own presets

Saving updates `presets.toml` (your comments in the file are kept) and re-registers the menu. The window is an app window of Edge, which every Windows 10/11 PC already has; Shock Convert serves the page only on this computer (`127.0.0.1`) from an address with a random key, and stops when the window closes.

## Presets

`shock-convert presets` prints the file path and creates a commented sample if it doesn't exist (`%APPDATA%\ShockConvert\presets.toml`). After editing, refresh the menu with `shock-convert register`. Built-in presets are always there; they can be replaced with the same `id` or hidden with `hide = [...]`.

```toml
[[preset]]
id = "web-jpg"
label = "JPG (web, 1920px)"
output = "jpg"
input = ["image"]         # the "image" group or individual extensions
suffix = "web"            # photo.png -> photo-web.jpg
quality = 80
max_width = 1920          # aspect ratio is kept, never upscaled
background = "#ffffff"    # colour for transparent areas in JPG
```

A preset only appears for files that already have its output format if it resizes (`max_width`/`max_height`) or sets a `suffix` ("JPG → JPG" is pointless, "JPG → 1920px JPG" is not).

Earlier versions also had audio/video (`kind = "ffmpeg"`) and PDF (`kind = "pdf"`, `"pdf-tool"`) presets. These are no longer supported: they are skipped with a warning and can be deleted in the settings window.

## Command line

```
shock-convert list                       # menu entries
shock-convert presets                    # path of presets.toml (creates a sample if missing)
shock-convert run jpg a.png b.webp       # convert files to JPG
shock-convert custom a.png b.webp        # custom convert window
shock-convert register | unregister      # write / remove only the right-click menu
shock-convert install | uninstall        # install / remove completely
```

## Troubleshooting

A process started from the right-click menu has no console, so errors are written to `%LOCALAPPDATA%\ShockConvert\log.txt`. If something doesn't work, run `powershell -ExecutionPolicy Bypass -File .\diagnose.ps1`: it checks the registry, conversion and a simulated multi-selection, and copies a report to the clipboard.

## Conversion rules

| Topic | Behaviour |
|---|---|
| Transparency → JPG | Transparent areas are flattened onto white (configurable per preset) |
| Quality | JPG quality 90. WebP is written **lossless** |
| EXIF orientation | Applied, so phone photos come out the right way up |
| Metadata | EXIF (camera, date, location) and the ICC colour profile are kept in JPG/PNG/WebP |
| File dates | The output keeps the source's created/modified dates. Can be turned off in settings |
| ICO | Multiple sizes from 16 to 256 px; sizes larger than the source are skipped, aspect ratio is kept |
| Animated GIF/WebP | First frame only |
| Broken file | An error notification is shown and no partial output is left |
| Partial files | Output is first written under a temporary name (`photo.part.jpg`) and renamed to `photo.jpg` when done. Even if the PC shuts down or the process is killed, no half-written file is left under the real name; a leftover `.part` file can be deleted (the name is reused after an hour) |

## Plugins

The core is Rust; heavy or special jobs (e.g. AI upscaling) can be added as external programs. Create `<install folder>\plugins\<name>\plugin.toml`, then run `shock-convert register`:

```toml
name = "upscale"

[[entry]]
id = "2x"
label = "Upscale 2x"
extensions = ["png", "jpg"]      # file types the entry appears for
output_suffix = "2x"             # optional: photo.png -> photo-2x.png (otherwise photo (1).png)
output_ext = "png"
command = ["python", "{plugin_dir}/upscale.py", "{input}", "{output}"]
```

The `{input}`, `{output}` and `{plugin_dir}` placeholders are filled in. `{output}` is a temporary name (`photo.part.png`) that is moved to the real name when the plugin succeeds. The command can be any program (Python script, `.exe`, …); a non-zero exit code counts as failure and the last stderr line is shown in the notification. To remove a plugin, delete its folder and run `register`. Example: [`examples/plugins/grayscale`](examples/plugins/grayscale).

## Development

```
cargo test --workspace                                  # core, menu and plugin tests (Linux/Windows)
cargo check --workspace --target x86_64-pc-windows-gnu  # check the Windows code from Linux
.\build.ps1                                             # Windows: exe + installer
```

```
crates/core   image conversion (formats, options, output naming)
crates/cli    shock-convert.exe: commands, presets, menu model, plugins, registry, notifications, windows
site          website (React + Vite; see docs/SITE.md)
assets        app icon (generated by make_icon.py; icon.ico is embedded in the exe)
installer     Inno Setup script
```

Registry keys (all under `HKCU`): `Software\Classes\SystemFileAssociations\.<ext>\shell\ShockConvert` and `Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert`. `unregister` deletes only these and cleans up parent keys left empty.

## Releases

Releases are made automatically by GitHub Actions; no manual tagging is needed:

- **When a PR is opened** (from this repo): a **pre-release** named `pr-<number>` is created with the PR's installer for testing. It is refreshed on every push, linked from a PR comment and deleted when the PR closes.
- **When merged into main:** a patch release follows the last one (`v0.1.0` → `v0.1.1`), release notes are generated from the PRs, and the installer is also placed at the fixed download address on GitHub Pages.
  - A `release:minor` or `release:major` label on the PR bumps that part instead (`v0.2.0`, `v1.0.0`).
  - A `release:skip` label, or a change that only touches docs/site/workflows, makes no release.
  - If the version in `crates/cli/Cargo.toml` is raised by hand, that version is used.
- **After a stable release:** the `winget` job opens a pull request with the new version's manifest in [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs). Setup and the first submission: [docs/WINGET.md](docs/WINGET.md).

## License

See [LICENSE](LICENSE) (Apache-2.0).
