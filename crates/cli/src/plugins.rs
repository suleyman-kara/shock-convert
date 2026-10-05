//! Eklentiler: `plugins/<ad>/plugin.toml` dosyalarıyla tanımlanan harici dönüştürücüler.
//!
//! ```toml
//! name = "upscale"
//! [[entry]]
//! id = "2x"
//! label = "Upscale 2x"
//! extensions = ["png", "jpg"]
//! output_suffix = "2x"
//! output_ext = "png"
//! command = ["python", "{plugin_dir}/upscale.py", "{input}", "{output}"]
//! ```

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    name: String,
    #[serde(default, rename = "entry")]
    entries: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    id: String,
    label: String,
    extensions: Vec<String>,
    output_suffix: String,
    output_ext: String,
    command: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub id: String,
    pub label: String,
    pub extensions: Vec<String>,
    pub output_suffix: String,
    pub output_ext: String,
    pub command: Vec<String>,
    pub plugin_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub entries: Vec<PluginEntry>,
}

/// `plugins_dir` altındaki tüm geçerli eklentileri yükler. Bozuk manifestler
/// atlanır; nedenleri ikinci dönüş değerinde uyarı olarak verilir.
pub fn load_all(plugins_dir: &Path) -> (Vec<Plugin>, Vec<String>) {
    let mut plugins = Vec::new();
    let mut warnings = Vec::new();

    let Ok(read) = std::fs::read_dir(plugins_dir) else {
        return (plugins, warnings);
    };
    let mut dirs: Vec<PathBuf> = read.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();

    for dir in dirs {
        let manifest_path = dir.join("plugin.toml");
        if !manifest_path.is_file() {
            continue;
        }
        match load_one(&dir, &manifest_path) {
            Ok(p) => plugins.push(p),
            Err(e) => warnings.push(format!("{}: {e}", manifest_path.display())),
        }
    }
    (plugins, warnings)
}

fn load_one(dir: &Path, manifest_path: &Path) -> Result<Plugin, String> {
    let text = std::fs::read_to_string(manifest_path).map_err(|e| e.to_string())?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| e.to_string())?;

    if manifest.name.is_empty() || manifest.name.contains(':') {
        return Err("`name` boş olamaz ve ':' içeremez".into());
    }
    let mut entries = Vec::new();
    for raw in manifest.entries {
        if raw.command.is_empty() || raw.extensions.is_empty() {
            return Err(format!("`{}` girdisinde command/extensions boş", raw.id));
        }
        entries.push(PluginEntry {
            id: raw.id,
            label: raw.label,
            extensions: raw.extensions,
            output_suffix: raw.output_suffix,
            output_ext: raw.output_ext,
            command: raw.command,
            plugin_dir: dir.to_path_buf(),
        });
    }
    Ok(Plugin {
        name: manifest.name,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_valid_and_reports_broken_manifests() {
        let root =
            std::env::temp_dir().join(format!("shock-convert-plugins-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("good")).unwrap();
        fs::create_dir_all(root.join("bad")).unwrap();
        fs::write(
            root.join("good/plugin.toml"),
            r#"name = "good"
[[entry]]
id = "x"
label = "X"
extensions = ["png"]
output_suffix = "x"
output_ext = "png"
command = ["tool", "{input}", "{output}"]
"#,
        )
        .unwrap();
        fs::write(root.join("bad/plugin.toml"), "name = ").unwrap();

        let (plugins, warnings) = load_all(&root);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].entries[0].plugin_dir, root.join("good"));
        assert_eq!(warnings.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
