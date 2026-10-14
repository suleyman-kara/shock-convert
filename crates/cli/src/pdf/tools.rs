//! Temel PDF işlemleri: birleştirme (PDF ve görüntüler), sayfalara bölme, döndürme.
//!
//! Birleştirme ve bölme, seçilen sayfalardan **sıfırdan temiz bir sayfa ağacı** kurar: sayfaların üst
//! düğümlerden miras aldığı özellikler (MediaBox, CropBox, Resources, Rotate) önce sayfanın kendisine
//! yazılır, sonra yeni bir Catalog/Pages altına bağlanır ve kullanılmayan nesneler budanır.
//! Kapsam dışı (ilk sürüm): yer imleri (outline), form alanları (AcroForm), belge bilgisi birleştirmede
//! korunmaz. Şifreli/parola korumalı PDF'ler reddedilir: yeni belge kurmak sahibinin koyduğu
//! kısıtlamaları sessizce kaldırırdı.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};
use shock_convert_core::{Format, image_to_pdf_bytes, unique_output_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Merge,
    Split,
    RotateRight,
    RotateLeft,
    Rotate180,
}

impl Tool {
    pub fn parse(s: &str) -> Option<Tool> {
        Some(match s {
            "merge" => Tool::Merge,
            "split" => Tool::Split,
            "rotate-right" => Tool::RotateRight,
            "rotate-left" => Tool::RotateLeft,
            "rotate-180" => Tool::Rotate180,
            _ => return None,
        })
    }

    fn degrees(self) -> i64 {
        match self {
            Tool::RotateRight => 90,
            Tool::RotateLeft => 270,
            Tool::Rotate180 => 180,
            _ => 0,
        }
    }
}

const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn load_pdf(path: &Path) -> Result<Document, String> {
    let doc = Document::load(path).map_err(|e| format!("PDF okunamadı: {e}"))?;
    if doc.trailer.has(b"Encrypt") || doc.encryption_state.is_some() {
        return Err("şifreli veya parola korumalı PDF'ler desteklenmiyor".into());
    }
    if doc.get_pages().is_empty() {
        return Err("PDF'te sayfa yok".into());
    }
    Ok(doc)
}

fn type_is(obj: &Object, name: &[u8]) -> bool {
    obj.as_dict()
        .ok()
        .and_then(|d| d.get(b"Type").ok())
        .and_then(|t| t.as_name().ok())
        == Some(name)
}

/// Sayfanın üst düğümlerden miras aldığı değeri bulur (sayfanın kendisi dahil).
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = Some(page);
    let mut guard = 0;
    while let Some(id) = current {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(v.clone());
        }
        current = dict.get(b"Parent").and_then(Object::as_reference).ok();
        guard += 1;
        if guard > 64 {
            return None; // döngüsel ağaç
        }
    }
    None
}

/// Miras alınan özellikleri sayfanın kendisine yazar (sayfa başka bir ağaca taşınabilsin).
fn materialize(doc: &mut Document, page: ObjectId) {
    let values: Vec<(&[u8], Object)> = INHERITABLE
        .iter()
        .filter_map(|k| inherited(doc, page, k).map(|v| (*k, v)))
        .collect();
    if let Ok(dict) = doc.get_dictionary_mut(page) {
        for (k, v) in values {
            if !dict.has(k) {
                dict.set(k.to_vec(), v);
            }
        }
    }
}

/// Sayfaları birden çok belgeden toplayıp yeni bir PDF kurar.
struct Builder {
    out: Document,
    kids: Vec<ObjectId>,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            out: Document::with_version("1.7"),
            kids: Vec::new(),
        }
    }

    /// `pages`: 1'den başlayan sayfa numaraları; `None` tüm sayfalar.
    fn append(&mut self, mut src: Document, pages: Option<&[u32]>) {
        src.renumber_objects_with(self.out.max_id + 1);
        let all = src.get_pages();
        let wanted: Vec<ObjectId> = match pages {
            None => all.values().copied().collect(),
            Some(ns) => ns.iter().filter_map(|n| all.get(n).copied()).collect(),
        };
        for &p in &wanted {
            materialize(&mut src, p);
        }
        let max_id = src.max_id;
        for (id, obj) in src.objects {
            let skip = type_is(&obj, b"Catalog")
                || type_is(&obj, b"Pages")
                || (type_is(&obj, b"Page") && !wanted.contains(&id));
            if !skip {
                self.out.objects.insert(id, obj);
            }
        }
        self.out.max_id = self.out.max_id.max(max_id);
        self.kids.extend(wanted);
    }

    fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.kids.is_empty() {
            return Err("çıktıda sayfa yok".into());
        }
        let pages_id = self.out.new_object_id();
        for kid in &self.kids {
            if let Ok(d) = self.out.get_dictionary_mut(*kid) {
                d.set("Parent", pages_id);
            }
        }
        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set(
            "Kids",
            self.kids
                .iter()
                .map(|k| Object::Reference(*k))
                .collect::<Vec<_>>(),
        );
        pages.set("Count", self.kids.len() as i64);
        self.out.objects.insert(pages_id, Object::Dictionary(pages));

        let mut catalog = Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog.set("Pages", pages_id);
        let catalog_id = self.out.add_object(catalog);
        self.out.trailer = Dictionary::new();
        self.out.trailer.set("Root", catalog_id);

        self.out.prune_objects();
        self.out.renumber_objects();
        let mut buf = Vec::new();
        self.out.save_to(&mut buf).map_err(err)?;
        Ok(buf)
    }
}

/// Çıktı adını rezerve edip yazar; hata olursa siler.
fn write_reserved(
    input: &Path,
    suffix: &str,
    bytes: Result<Vec<u8>, String>,
) -> Result<PathBuf, String> {
    let bytes = bytes?;
    let out = unique_output_path(input, Some(suffix), "pdf").map_err(err)?;
    std::fs::write(&out, bytes).map_err(|e| {
        let _ = std::fs::remove_file(&out);
        e.to_string()
    })?;
    Ok(out)
}

/// "sayfa2" < "sayfa10": sayılar sayı olarak, geri kalan büyük/küçük harf duyarsız karşılaştırılır.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (n, m) = (take(&mut x), take(&mut y));
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                match n.len().cmp(&m.len()).then_with(|| n.cmp(m)) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
            (Some(c), Some(d)) => {
                match c.to_lowercase().cmp(d.to_lowercase()) {
                    Ordering::Equal => {}
                    o => return o,
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Dosyaları (PDF ve görüntüler) ada göre doğal sırada tek PDF'te birleştirir.
/// Çıktı ilk dosyanın klasörüne `<ilk-ad>-<ek>.pdf` olarak yazılır.
pub fn merge(files: &[PathBuf], suffix: &str) -> Result<PathBuf, String> {
    let mut files: Vec<&PathBuf> = files.iter().collect();
    files.sort_by(|a, b| {
        let name = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        natural_cmp(&name(a), &name(b))
    });
    if files.len() < 2 {
        return Err("birleştirmek için en az iki dosya seçin".into());
    }
    let mut builder = Builder::new();
    for f in &files {
        let ext = f
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let doc = if ext == "pdf" {
            load_pdf(f)
        } else if Format::from_ext(&ext).is_some() {
            image_to_pdf_bytes(f)
                .map_err(err)
                .and_then(|b| Document::load_mem(&b).map_err(err))
        } else {
            Err(format!("`.{ext}` birleştirilemez"))
        };
        let doc = doc.map_err(|e| {
            format!(
                "{}: {e}",
                f.file_name().unwrap_or_default().to_string_lossy()
            )
        })?;
        builder.append(doc, None);
    }
    write_reserved(files[0], suffix, builder.finish())
}

/// Her sayfayı ayrı bir PDF olarak yazar: `<ad>-<ek>-01.pdf`, `-02` … Yazılan ilk dosyayı döndürür.
pub fn split(file: &Path, suffix: &str) -> Result<PathBuf, String> {
    let doc = load_pdf(file)?;
    let count = doc.get_pages().len() as u32;
    if count < 2 {
        return Err("tek sayfalık PDF bölünemez".into());
    }
    let width = count.to_string().len();
    let mut first = None;
    for n in 1..=count {
        let mut b = Builder::new();
        b.append(doc.clone(), Some(&[n]));
        let out = write_reserved(file, &format!("{suffix}-{n:0width$}"), b.finish())?;
        first.get_or_insert(out);
    }
    first.ok_or_else(|| "sayfa yazılamadı".into())
}

/// Tüm sayfaları döndürür; belgenin geri kalanı (yer imleri, formlar) olduğu gibi kalır.
pub fn rotate(file: &Path, tool: Tool, suffix: &str) -> Result<PathBuf, String> {
    let mut doc = load_pdf(file)?;
    let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for p in pages {
        let current = inherited(&doc, p, b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let next = (current + tool.degrees()).rem_euclid(360);
        if let Ok(d) = doc.get_dictionary_mut(p) {
            d.set("Rotate", next);
        }
    }
    let mut buf = Vec::new();
    let saved = doc.save_to(&mut buf).map(|_| buf).map_err(err);
    write_reserved(file, suffix, saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::dictionary;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "shock-convert-pdftools-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// `labels` kadar sayfalı PDF; her sayfada etiket metni. MediaBox ve Rotate **üst düğümde**
    /// (miras) tanımlanır ki taşıma sırasında kaybolmadıkları sınansın.
    fn make_pdf(path: &Path, labels: &[&str], parent_rotate: i64) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
        let mut kids = vec![];
        for l in labels {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![50.into(), 700.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*l)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let c = doc.add_object(lopdf::Stream::new(
                dictionary! {},
                content.encode().unwrap(),
            ));
            kids.push(
                doc.add_object(
                    dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => c },
                )
                .into(),
            );
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => labels.len() as i64,
                "Resources" => resources, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Rotate" => parent_rotate,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    fn page_texts(path: &Path) -> Vec<String> {
        let doc = Document::load(path).unwrap();
        doc.get_pages()
            .keys()
            .map(|n| {
                doc.extract_text(&[*n])
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .collect()
    }

    fn page_attr(path: &Path, page: u32, key: &[u8]) -> Option<Object> {
        let doc = Document::load(path).unwrap();
        let id = doc.get_pages()[&page];
        inherited(&doc, id, key)
    }

    #[test]
    fn merge_keeps_order_inherited_attributes_and_accepts_images() {
        let d = dir("merge");
        make_pdf(&d.join("bolum10.pdf"), &["J"], 0);
        make_pdf(&d.join("bolum2.pdf"), &["C", "D"], 90);
        make_pdf(&d.join("bolum1.pdf"), &["A", "B"], 0);
        image::RgbaImage::from_pixel(20, 10, image::Rgba([1, 2, 3, 255]))
            .save(d.join("bolum3.png"))
            .unwrap();
        let files: Vec<PathBuf> = ["bolum10.pdf", "bolum2.pdf", "bolum3.png", "bolum1.pdf"]
            .iter()
            .map(|f| d.join(f))
            .collect();

        let out = merge(&files, "birleşik").unwrap();
        assert_eq!(out, d.join("bolum1-birleşik.pdf"));
        // Doğal sıra: 1, 2, 3 (görüntü), 10
        assert_eq!(page_texts(&out), ["A", "B", "C", "D", "", "J"]);
        // bolum2'nin üst düğümden miras aldığı 90° döndürme ve sayfa boyutu korunur
        assert_eq!(
            page_attr(&out, 3, b"Rotate").and_then(|o| o.as_i64().ok()),
            Some(90)
        );
        assert!(page_attr(&out, 1, b"MediaBox").is_some());
        assert!(page_attr(&out, 1, b"Resources").is_some());
        assert_eq!(Document::load(&out).unwrap().get_pages().len(), 6);
    }

    #[test]
    fn split_writes_one_numbered_file_per_page() {
        let d = dir("split");
        let src = d.join("rapor.pdf");
        make_pdf(&src, &["Bir", "Iki", "Uc"], 0);
        let first = split(&src, "sayfa").unwrap();
        assert_eq!(first, d.join("rapor-sayfa-1.pdf"));
        for (n, label) in [(1, "Bir"), (2, "Iki"), (3, "Uc")] {
            let p = d.join(format!("rapor-sayfa-{n}.pdf"));
            assert_eq!(page_texts(&p), [label]);
            assert!(page_attr(&p, 1, b"MediaBox").is_some());
        }
        let one = d.join("tek.pdf");
        make_pdf(&one, &["X"], 0);
        assert!(split(&one, "sayfa").is_err());
    }

    #[test]
    fn rotate_adds_to_inherited_rotation() {
        let d = dir("rotate");
        let src = d.join("tablo.pdf");
        make_pdf(&src, &["A", "B"], 90);
        let out = rotate(&src, Tool::RotateRight, "döndürülmüş").unwrap();
        assert_eq!(out, d.join("tablo-döndürülmüş.pdf"));
        assert_eq!(
            page_attr(&out, 2, b"Rotate").and_then(|o| o.as_i64().ok()),
            Some(180)
        );
        let left = rotate(&src, Tool::RotateLeft, "döndürülmüş").unwrap();
        assert_eq!(
            page_attr(&left, 1, b"Rotate").and_then(|o| o.as_i64().ok()),
            Some(0)
        );
        assert_eq!(page_texts(&left), ["A", "B"]);
    }

    #[test]
    fn rejects_corrupt_and_too_few_inputs() {
        let d = dir("bad");
        let bad = d.join("bozuk.pdf");
        std::fs::write(&bad, b"%PDF-1.4 bozuk").unwrap();
        assert!(
            rotate(&bad, Tool::Rotate180, "x")
                .unwrap_err()
                .contains("PDF okunamadı")
        );
        assert!(
            merge(std::slice::from_ref(&bad), "x")
                .unwrap_err()
                .contains("en az iki")
        );
        assert!(!d.join("bozuk-x.pdf").exists());
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["s10.pdf", "S2.pdf", "s1.pdf", "s02b.pdf"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["s1.pdf", "S2.pdf", "s02b.pdf", "s10.pdf"]);
    }
}
