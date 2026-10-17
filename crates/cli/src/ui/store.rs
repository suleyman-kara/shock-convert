//! Ayarlar arayüzünün okuduğu ve yazdığı `presets.toml`. Yazarken dosyadaki yorumlar ve
//! bilinmeyen anahtarlar korunur (`toml_edit`); yalnızca arayüzün yönettiği anahtarlar değişir.
//! Varsayılan değerler dosyaya yazılmaz, dosya kısa ve elle okunabilir kalır.

use serde::{Deserialize, Serialize};
use toml_edit::{Array, DocumentMut, Item, value};

use crate::config;
use crate::presets::{self, RawPreset};

/// Arayüzle alışverişi yapılan ayarlar (JSON).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Values {
    pub keep_dates: bool,
    /// keep | strip-location | strip
    pub metadata: String,
    /// Menüden gizlenen yerleşik preset kimlikleri.
    pub hide: Vec<String>,
    /// Kullanıcının kendi preset'leri (aynı kimlikle yerleşik olanı ezenler dahil).
    pub presets: Vec<RawPreset>,
}

impl Values {
    pub fn current() -> Result<Values, String> {
        let f = presets::read_file()?;
        Ok(Values {
            keep_dates: f.keep_dates.unwrap_or(true),
            metadata: f.metadata.unwrap_or_else(|| "keep".into()),
            hide: f.hide,
            presets: f.preset,
        })
    }

    /// Kaydetmeden önce tüm hataları toplar: (alan ya da preset kimliği, ileti).
    pub fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if shock_convert_core::Metadata::parse(&self.metadata).is_none() {
            errors.push((
                "metadata".into(),
                format!("invalid value: {}", self.metadata),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.presets {
            if !seen.insert(p.id.to_ascii_lowercase()) {
                errors.push((p.id.clone(), "this ID is used more than once".into()));
            }
            if let Err(e) = p.clone().into_preset() {
                errors.push((p.id.clone(), e));
            }
        }
        errors
    }
}

fn set_or_remove(doc: &mut DocumentMut, key: &str, item: Option<Item>) {
    match item {
        Some(i) => {
            doc[key] = i;
        }
        None => {
            doc.remove(key);
        }
    }
}

/// Dosyanın yeni metnini üretir (yazmadan; test edilebilsin).
pub fn render(existing: &str, v: &Values) -> Result<String, String> {
    let mut doc: DocumentMut = existing
        .parse()
        .map_err(|e| format!("could not read presets.toml: {e}"))?;
    // Yalnızca yorumdan oluşan dosya (ilk kurulumdaki açıklamalı örnek): yorumlar başta kalsın,
    // ayarlar ve preset'ler altına eklensin.
    if doc.iter().next().is_none() && !existing.trim().is_empty() {
        let body = render("", v)?;
        return Ok(if body.trim().is_empty() {
            existing.to_string()
        } else {
            format!("{}\n\n{body}", existing.trim_end())
        });
    }
    set_or_remove(
        &mut doc,
        "keep_dates",
        (!v.keep_dates).then(|| value(false)),
    );
    set_or_remove(
        &mut doc,
        "metadata",
        (v.metadata != "keep").then(|| value(v.metadata.as_str())),
    );
    let hide: Array = v.hide.iter().map(String::as_str).collect();
    set_or_remove(&mut doc, "hide", (!hide.is_empty()).then(|| value(hide)));

    // `[[preset]]` tablolarının üstündeki yorumlar tabloya aittir; tablolar yeniden yazılırken
    // kaybolmasınlar diye ilk tablonun önündeki yorum taşınır.
    let leading = match doc.remove("preset") {
        Some(Item::ArrayOfTables(old)) => old
            .get(0)
            .and_then(|t| t.decor().prefix())
            .and_then(|p| p.as_str())
            .map(str::to_string),
        _ => None,
    };
    if v.presets.is_empty() {
        if let Some(c) = leading.filter(|c| !c.trim().is_empty()) {
            let trailing = doc.trailing().as_str().unwrap_or_default().to_string();
            doc.set_trailing(format!("{c}{trailing}"));
        }
    } else {
        #[derive(Serialize)]
        struct Wrapper<'a> {
            preset: &'a [RawPreset],
        }
        let text = toml::to_string(&Wrapper { preset: &v.presets }).map_err(|e| e.to_string())?;
        let mut parsed: DocumentMut = text.parse().map_err(|e| format!("{e}"))?;
        if let Some(mut item) = parsed.remove("preset") {
            if let (Some(c), Some(first)) = (
                leading,
                item.as_array_of_tables_mut().and_then(|a| a.get_mut(0)),
            ) {
                first.decor_mut().set_prefix(c);
            }
            doc["preset"] = item;
        }
    }
    Ok(doc.to_string())
}

/// Doğrular ve yazar. Yazma atomiktir: önce geçici dosya, sonra yeniden adlandırma.
pub fn save(v: &Values) -> Result<(), Vec<(String, String)>> {
    let errors = v.validate();
    if !errors.is_empty() {
        return Err(errors);
    }
    let fail = |e: String| vec![(String::new(), e)];
    let path = config::presets_path();
    let existing = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => presets::sample().to_string(),
        Err(e) => return Err(fail(e.to_string())),
    };
    let text = render(&existing, v).map_err(fail)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(e.to_string()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fail(e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values() -> Values {
        Values {
            keep_dates: true,
            metadata: "keep".into(),
            hide: vec![],
            presets: vec![],
        }
    }

    fn web_jpg() -> RawPreset {
        RawPreset {
            id: "web-jpg".into(),
            label: "JPG (web, 1920 px)".into(),
            kind: "image".into(),
            output: "jpg".into(),
            input: vec!["image".into()],
            suffix: Some("web".into()),
            quality: Some(80),
            max_width: Some(1920),
            max_height: None,
            background: None,
        }
    }

    #[test]
    fn keeps_comments_and_writes_only_non_defaults() {
        let existing = "# benim notum\n# keep_dates = true\nozel = 1\npdf_engine = \"office\"\n";
        let mut v = values();
        v.keep_dates = false;
        v.metadata = "strip-location".into();
        v.hide = vec!["bmp".into()];
        v.presets = vec![web_jpg()];
        let out = render(existing, &v).unwrap();
        assert!(out.contains("# benim notum"), "{out}");
        assert!(
            out.contains("ozel = 1"),
            "bilinmeyen anahtar korunmalı: {out}"
        );
        assert!(out.contains("keep_dates = false"));
        assert!(out.contains("metadata = \"strip-location\""));
        assert!(out.contains("pdf_engine = \"office\""), "eski sürümün anahtarı korunmalı: {out}");
        assert!(out.contains("[[preset]]") && out.contains("max_width = 1920"));
        assert!(!out.contains("max_height"), "boş alan yazılmaz: {out}");
        let back: crate::presets::FileModel = toml::from_str(&out).unwrap();
        assert_eq!(back.preset.len(), 1);
        assert_eq!(back.hide, ["bmp"]);

        // Varsayılana dönünce anahtarlar silinir; yorum yine durur.
        let again = render(&out, &values()).unwrap();
        assert!(
            !again.lines().any(|l| l.starts_with("keep_dates")) && !again.contains("[[preset]]"),
            "{again}"
        );
        assert!(again.contains("# benim notum"));
    }

    #[test]
    fn comment_above_presets_survives_rewrite_and_removal() {
        let existing = "# Benim notum: bunu silme\n[[preset]]\nid = \"eski\"\nlabel = \"Eski\"\nkind = \"image\"\noutput = \"png\"\ninput = [\"image\"]\n";
        let mut v = values();
        v.keep_dates = false;
        v.presets = vec![web_jpg()];
        let out = render(existing, &v).unwrap();
        assert!(out.contains("# Benim notum: bunu silme"), "{out}");
        assert!(!out.contains("eski") && out.contains("web-jpg"));
        let none = render(existing, &values()).unwrap();
        assert!(none.contains("# Benim notum: bunu silme"), "{none}");
        assert!(!none.contains("[[preset]]"));
    }

    #[test]
    fn comment_only_sample_keeps_its_header_on_top() {
        let sample = crate::presets::sample();
        let mut v = values();
        v.keep_dates = false;
        v.presets = vec![web_jpg()];
        let out = render(sample, &v).unwrap();
        assert!(out.starts_with("# Shock Convert"), "{out:.200}");
        let header = out.find("# Shock Convert").unwrap();
        assert!(
            out.find("[[preset]]").unwrap() > header
                && out.find("keep_dates = false").unwrap() > header
        );
        let back: crate::presets::FileModel = toml::from_str(&out).unwrap();
        assert_eq!((back.preset.len(), back.keep_dates), (1, Some(false)));
        assert_eq!(
            render(sample, &values()).unwrap(),
            sample,
            "değişiklik yoksa dosya aynı kalır"
        );
    }

    #[test]
    fn validation_reports_every_bad_field() {
        let mut v = values();
        v.metadata = "sil".into();
        let mut bad = web_jpg();
        bad.output = "exe".into();
        v.presets = vec![web_jpg(), web_jpg(), bad];
        let errors = v.validate();
        assert!(errors.iter().any(|(k, _)| k == "metadata"));
        assert!(errors.iter().any(|(_, e)| e.contains("more than once")));
        assert!(errors.iter().any(|(_, e)| e.contains("exe")), "{errors:?}");
    }
}
