//! XLSX -> HTML (Office olmadan). Her çalışma sayfası bir tablo olur. Hücre değerleri ham gösterilir;
//! tarih biçimli hücreler tarihe çevrilir. Sayı biçimleri (para birimi, ondalık), formül sonuçları dışındaki
//! hesaplamalar, grafikler ve hücre biçimlendirmesi desteklenmez.

use std::collections::BTreeMap;
use std::path::Path;

use super::html::{escape, page};
use super::ooxml::{Package, resolve};
use super::xml::{self, Ev};

/// Çok büyük sayfaların PDF'i devasa yapmasını önler.
const MAX_CELLS: usize = 300_000;

struct Sheet {
    name: String,
    cells: BTreeMap<(u32, u32), (String, bool)>, // (satır, sütun) -> (değer, sayısal mı)
    truncated: bool,
}

/// `B12` -> (satır 12, sütun 2); satır/sütun 1'den başlar.
fn parse_ref(r: &str) -> Option<(u32, u32)> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &r[letters.len()..];
    if letters.is_empty() {
        return None;
    }
    let col = letters
        .to_ascii_uppercase()
        .bytes()
        .fold(0u32, |acc, b| acc * 26 + u32::from(b - b'A' + 1));
    Some((digits.parse().ok()?, col))
}

/// Excel seri numarası -> `YYYY-AA-GG` (kesir varsa saat de eklenir). 1900 artık yıl hatası dahil.
fn serial_to_date(serial: f64) -> Option<String> {
    if !(0.0..2_958_466.0).contains(&serial) {
        return None;
    }
    let days = serial.floor() as i64;
    let secs = ((serial - serial.floor()) * 86_400.0).round() as i64;
    // Excel dönemi 1899-12-30; 25569 = Unix epoch (1970-01-01) serisi. Sivil takvim: Howard Hinnant algoritması.
    let unix_days = days - 25_569;
    let z = unix_days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let mut s = format!("{y:04}-{m:02}-{d:02}");
    if secs != 0 {
        s.push_str(&format!(" {:02}:{:02}", secs / 3600, secs % 3600 / 60));
    }
    Some(s)
}

fn is_date_format(code: &str) -> bool {
    let mut cleaned = String::new();
    let (mut in_quote, mut in_bracket, mut escaped) = (false, false, false);
    for c in code.chars() {
        match (c, in_quote, in_bracket, escaped) {
            (_, _, _, true) => escaped = false,
            ('\\', _, _, false) => escaped = true,
            ('"', _, _, _) => in_quote = !in_quote,
            ('[', false, _, _) => in_bracket = true,
            (']', false, true, _) => in_bracket = false,
            (_, false, false, _) => cleaned.push(c.to_ascii_lowercase()),
            _ => {}
        }
    }
    cleaned.contains(['y', 'd', 'h', 's']) && !cleaned.contains("general")
}

fn builtin_date_id(id: u32) -> bool {
    matches!(id, 14..=22 | 27..=36 | 45..=47 | 50..=58)
}

/// cellXfs sırası -> tarih biçimli mi
fn parse_date_styles(xml_text: &str) -> Vec<bool> {
    let mut custom: BTreeMap<u32, bool> = BTreeMap::new();
    let mut xfs: Vec<u32> = Vec::new();
    let mut in_cell_xfs = false;
    let _ = xml::parse(xml_text, |ev| {
        match ev {
            Ev::Start(t) if t.name == "numFmt" => {
                if let (Some(id), Some(code)) = (
                    t.get("numFmtId").and_then(|v| v.parse().ok()),
                    t.get("formatCode"),
                ) {
                    custom.insert(id, is_date_format(code));
                }
            }
            Ev::Start(t) if t.name == "cellXfs" => in_cell_xfs = true,
            Ev::End(n) if n == "cellXfs" => in_cell_xfs = false,
            Ev::Start(t) if t.name == "xf" && in_cell_xfs => {
                xfs.push(t.get("numFmtId").and_then(|v| v.parse().ok()).unwrap_or(0));
            }
            _ => {}
        }
        true
    });
    xfs.into_iter()
        .map(|id| {
            custom
                .get(&id)
                .copied()
                .unwrap_or_else(|| builtin_date_id(id))
        })
        .collect()
}

fn parse_shared_strings(xml_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut cur, mut in_t, mut in_phonetic) = (String::new(), false, false);
    let _ = xml::parse(xml_text, |ev| {
        match ev {
            Ev::Start(t) if t.name == "si" => cur.clear(),
            Ev::Start(t) if t.name == "rPh" => in_phonetic = true,
            Ev::End(n) if n == "rPh" => in_phonetic = false,
            Ev::Start(t) if t.name == "t" => in_t = true,
            Ev::End(n) if n == "t" => in_t = false,
            Ev::End(n) if n == "si" => out.push(std::mem::take(&mut cur)),
            Ev::Text(s) if in_t && !in_phonetic => cur.push_str(&s),
            _ => {}
        }
        true
    });
    out
}

fn parse_sheet(xml_text: &str, name: String, shared: &[String], date_styles: &[bool]) -> Sheet {
    let mut cells = BTreeMap::new();
    let mut truncated = false;
    // Geçerli hücre durumu
    let (mut pos, mut kind, mut style): (Option<(u32, u32)>, String, usize) =
        (None, String::new(), 0);
    let (mut v, mut inline, mut in_v, mut in_is_t) = (String::new(), String::new(), false, false);

    let _ = xml::parse(xml_text, |ev| {
        match ev {
            Ev::Start(t) if t.name == "c" => {
                pos = t.get("r").and_then(parse_ref);
                kind = t.get("t").unwrap_or("n").to_string();
                style = t.get("s").and_then(|x| x.parse().ok()).unwrap_or(0);
                v.clear();
                inline.clear();
            }
            Ev::Start(t) if t.name == "v" => in_v = true,
            Ev::End(n) if n == "v" => in_v = false,
            Ev::Start(t) if t.name == "t" => in_is_t = true,
            Ev::End(n) if n == "t" => in_is_t = false,
            Ev::Text(s) if in_v => v.push_str(&s),
            Ev::Text(s) if in_is_t => inline.push_str(&s),
            Ev::End(n) if n == "c" => {
                if let Some(p) = pos.take() {
                    let numeric_kind = matches!(kind.as_str(), "n");
                    let (text, numeric) = match kind.as_str() {
                        "s" => (
                            v.trim()
                                .parse::<usize>()
                                .ok()
                                .and_then(|i| shared.get(i))
                                .cloned()
                                .unwrap_or_default(),
                            false,
                        ),
                        "inlineStr" => (inline.clone(), false),
                        "b" => (
                            (if v.trim() == "1" { "TRUE" } else { "FALSE" }).to_string(),
                            false,
                        ),
                        _ if numeric_kind && date_styles.get(style).copied().unwrap_or(false) => {
                            match v.trim().parse::<f64>().ok().and_then(serial_to_date) {
                                Some(d) => (d, false),
                                None => (v.trim().to_string(), true),
                            }
                        }
                        _ => (v.trim().to_string(), numeric_kind),
                    };
                    if !text.is_empty() {
                        if cells.len() >= MAX_CELLS {
                            truncated = true;
                            return false;
                        }
                        cells.insert(p, (text, numeric));
                    }
                }
            }
            _ => {}
        }
        true
    });
    Sheet {
        name,
        cells,
        truncated,
    }
}

fn render(sheet: &Sheet, out: &mut String) {
    out.push_str(&format!(
        "<section class=\"sheet\"><h2>{}</h2>",
        escape(&sheet.name)
    ));
    if sheet.cells.is_empty() {
        out.push_str("<p><i>(boş sayfa)</i></p></section>");
        return;
    }
    let (min_c, max_c) = sheet
        .cells
        .keys()
        .fold((u32::MAX, 0), |(lo, hi), &(_, c)| (lo.min(c), hi.max(c)));
    out.push_str("<table>");
    // satır satır: aynı satırdaki hücreleri sütun sırasıyla yaz
    let mut rows: BTreeMap<u32, BTreeMap<u32, &(String, bool)>> = BTreeMap::new();
    for (&(r, c), v) in &sheet.cells {
        rows.entry(r).or_default().insert(c, v);
    }
    for row in rows.values() {
        out.push_str("<tr>");
        for c in min_c..=max_c {
            match row.get(&c) {
                Some((text, true)) => {
                    out.push_str(&format!("<td class=\"n\">{}</td>", escape(text)))
                }
                Some((text, false)) => out.push_str(&format!("<td>{}</td>", escape(text))),
                None => out.push_str("<td></td>"),
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    if sheet.truncated {
        out.push_str(&format!(
            "<p><i>Not: çok büyük sayfa; ilk {MAX_CELLS} hücre gösterildi.</i></p>"
        ));
    }
    out.push_str("</section>");
}

pub fn to_html(path: &Path) -> Result<String, String> {
    let mut pkg = Package::open(path)?;
    let workbook = pkg
        .read_string("xl/workbook.xml")
        .ok_or("xl/workbook.xml yok: geçerli bir XLSX değil")?;
    let rels = pkg.rels_of("xl/workbook.xml");
    let shared = pkg
        .read_string("xl/sharedStrings.xml")
        .map(|s| parse_shared_strings(&s))
        .unwrap_or_default();
    let date_styles = pkg
        .read_string("xl/styles.xml")
        .map(|s| parse_date_styles(&s))
        .unwrap_or_default();

    let mut sheets: Vec<(String, String)> = Vec::new(); // (ad, parça yolu)
    let _ = xml::parse(&workbook, |ev| {
        if let Ev::Start(t) = ev
            && t.name == "sheet"
            && !matches!(t.get("state"), Some("hidden" | "veryHidden"))
            && let (Some(name), Some(rel)) =
                (t.get("name"), t.rel("id").and_then(|id| rels.get(id)))
        {
            sheets.push((name.to_string(), resolve("xl", &rel.target)));
        }
        true
    });
    if sheets.is_empty() {
        return Err("çalışma sayfası bulunamadı".into());
    }

    let mut body = String::new();
    for (name, part) in sheets {
        let Some(xml_text) = pkg.read_string(&part) else {
            continue;
        };
        render(
            &parse_sheet(&xml_text, name, &shared, &date_styles),
            &mut body,
        );
    }
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(page(&title, &body, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_refs_and_dates() {
        assert_eq!(parse_ref("B12"), Some((12, 2)));
        assert_eq!(parse_ref("AA3"), Some((3, 27)));
        assert_eq!(serial_to_date(45_000.0).as_deref(), Some("2023-03-15"));
        assert_eq!(
            serial_to_date(44_927.5).as_deref(),
            Some("2023-01-01 12:00")
        );
        assert!(
            is_date_format("dd.mm.yyyy")
                && is_date_format("[$-409]d-mmm-yy")
                && !is_date_format("0.00")
                && !is_date_format("#,##0 \"gün\"")
        );
        assert!(builtin_date_id(14) && !builtin_date_id(2));
    }

    #[test]
    fn workbook_renders_visible_sheets_with_values_and_dates() {
        let path = crate::pdf::fixtures::temp_file("tablo.xlsx");
        crate::pdf::fixtures::xlsx(&path);
        let html = to_html(&path).unwrap();
        assert!(html.contains("<h2>Satış</h2>"));
        assert!(!html.contains("GİZLİ"), "gizli sayfa basılmamalı");
        assert!(
            html.contains("<td>Şehir</td>") && html.contains("<td>İstanbul</td>"),
            "{html}"
        );
        assert!(
            !html.contains("İstanbulX"),
            "fonetik metin (rPh) eklenmemeli"
        );
        assert!(html.contains("<td>Not &lt;1&gt;</td>"));
        assert!(html.contains("<td class=\"n\">1250.5</td>"));
        assert!(
            html.contains("<td>2023-03-15</td>"),
            "özel tarih biçimi: {html}"
        );
        assert!(html.contains("<td>TRUE</td>"));
        // Boş satır 2 basılmaz; değersiz E3 sütun sınırına girmez: A..D, 2 satır x 4 sütun
        assert_eq!(html.matches("<tr>").count(), 2);
        assert_eq!(html.matches("<td").count(), 8);
    }
}
