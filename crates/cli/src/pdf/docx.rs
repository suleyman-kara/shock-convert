//! DOCX -> HTML (Office olmadan). Kapsam: başlıklar, kalın/italik/altı çizili/üstü çizili, renk ve boyut,
//! hizalama, madde/numaralı listeler, tablolar (gridSpan), köprüler, satır içi resimler, sayfa sonları.
//! Kapsam dışı: üst/alt bilgi, dipnot, metin kutuları, şekiller, değişiklik izleme, birebir sayfa düzeni.

use std::collections::HashMap;
use std::path::Path;

use super::html::{escape, page};
use super::ooxml::{Package, Rel};
use super::xml::{self, Ev, Tag};

#[derive(Default, Clone)]
struct Run {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    sup: bool,
    sub: bool,
    color: Option<String>,
    size_pt: Option<f32>,
}

#[derive(Default)]
struct Para {
    heading: Option<u8>,
    align: Option<&'static str>,
    num: Option<(String, usize)>,
    html: String,
    page_break_after: bool,
}

fn on(tag: &Tag) -> bool {
    !matches!(tag.get("val"), Some("0" | "false" | "off" | "none"))
}

/// stil kimliği -> başlık düzeyi (1-6; Title/Subtitle için 1/2).
fn parse_styles(xml_text: &str) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    let mut id: Option<String> = None;
    let _ = xml::parse(xml_text, |ev| {
        match ev {
            Ev::Start(t) if t.name == "style" => id = t.get("styleId").map(String::from),
            Ev::Start(t) if t.name == "name" => {
                if let (Some(id), Some(name)) = (&id, t.get("val")) {
                    let n = name.to_ascii_lowercase();
                    let level = if let Some(rest) = n.strip_prefix("heading ") {
                        rest.trim()
                            .parse::<u8>()
                            .ok()
                            .filter(|l| (1..=6).contains(l))
                    } else if n == "title" {
                        Some(1)
                    } else if n == "subtitle" {
                        Some(2)
                    } else {
                        None
                    };
                    if let Some(l) = level {
                        map.insert(id.clone(), l);
                    }
                }
            }
            Ev::End(n) if n == "style" => id = None,
            _ => {}
        }
        true
    });
    map
}

/// numId -> (ilvl -> numFmt)
fn parse_numbering(xml_text: &str) -> HashMap<String, HashMap<usize, String>> {
    let mut abstract_fmt: HashMap<String, HashMap<usize, String>> = HashMap::new();
    let mut num_to_abs: HashMap<String, String> = HashMap::new();
    let (mut cur_abs, mut cur_lvl, mut cur_num): (Option<String>, Option<usize>, Option<String>) =
        (None, None, None);
    let _ = xml::parse(xml_text, |ev| {
        match ev {
            Ev::Start(t) if t.name == "abstractNum" => {
                cur_abs = t.get("abstractNumId").map(String::from)
            }
            Ev::Start(t) if t.name == "lvl" => cur_lvl = t.get("ilvl").and_then(|v| v.parse().ok()),
            Ev::Start(t) if t.name == "numFmt" => {
                if let (Some(a), Some(l), Some(v)) = (&cur_abs, cur_lvl, t.get("val")) {
                    abstract_fmt
                        .entry(a.clone())
                        .or_default()
                        .insert(l, v.to_string());
                }
            }
            Ev::Start(t) if t.name == "num" => cur_num = t.get("numId").map(String::from),
            Ev::Start(t) if t.name == "abstractNumId" => {
                if let (Some(n), Some(v)) = (&cur_num, t.get("val")) {
                    num_to_abs.insert(n.clone(), v.to_string());
                }
            }
            Ev::End(n) if n == "abstractNum" => cur_abs = None,
            Ev::End(n) if n == "lvl" => cur_lvl = None,
            Ev::End(n) if n == "num" => cur_num = None,
            _ => {}
        }
        true
    });
    num_to_abs
        .into_iter()
        .filter_map(|(num, abs)| abstract_fmt.get(&abs).map(|f| (num, f.clone())))
        .collect()
}

fn roman(mut n: u32) -> String {
    const T: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn marker(fmt: &str, n: u32) -> String {
    let letter = |upper: bool| {
        let base = if upper { b'A' } else { b'a' };
        let mut k = n - 1;
        let mut out = String::new();
        loop {
            out.insert(0, (base + (k % 26) as u8) as char);
            if k < 26 {
                break;
            }
            k = k / 26 - 1;
        }
        out
    };
    match fmt {
        "bullet" | "none" => "•".to_string(),
        "lowerLetter" => format!("{}.", letter(false)),
        "upperLetter" => format!("{}.", letter(true)),
        "lowerRoman" => format!("{}.", roman(n).to_lowercase()),
        "upperRoman" => format!("{}.", roman(n)),
        _ => format!("{n}."),
    }
}

/// `html` zaten kaçışlanmış içeriktir (metin, `&emsp;`, `<br>`); stil sarmalayıcıları eklenir.
fn wrap_run(html: &str, r: &Run) -> String {
    if html.is_empty() {
        return String::new();
    }
    let mut s = html.to_string();
    let mut style = String::new();
    if let Some(c) = &r.color {
        style.push_str(&format!("color:#{c};"));
    }
    if let Some(sz) = r.size_pt {
        style.push_str(&format!("font-size:{sz}pt;"));
    }
    if !style.is_empty() {
        s = format!("<span style=\"{style}\">{s}</span>");
    }
    for (flag, tag) in [
        (r.bold, "b"),
        (r.italic, "i"),
        (r.underline, "u"),
        (r.strike, "s"),
        (r.sup, "sup"),
        (r.sub, "sub"),
    ] {
        if flag {
            s = format!("<{tag}>{s}</{tag}>");
        }
    }
    s
}

/// Çözümlenmiş DOCX -> tam HTML sayfası.
pub fn to_html(path: &Path) -> Result<String, String> {
    let mut pkg = Package::open(path)?;
    let doc = pkg
        .read_string("word/document.xml")
        .ok_or("word/document.xml yok: geçerli bir DOCX değil")?;
    let styles = pkg
        .read_string("word/styles.xml")
        .map(|s| parse_styles(&s))
        .unwrap_or_default();
    let numbering = pkg
        .read_string("word/numbering.xml")
        .map(|s| parse_numbering(&s))
        .unwrap_or_default();
    let rels: HashMap<String, Rel> = pkg.rels_of("word/document.xml");

    let mut out = String::new();
    let mut para: Option<Para> = None;
    let mut run = Run::default();
    let mut run_html = String::new();
    let mut in_text = false;
    let mut in_run_props = false;
    let mut in_ppr = false;
    let mut counters: HashMap<(String, usize), u32> = HashMap::new();
    let mut link_open = false;
    let mut cell_depth = 0usize;
    let mut extent_px: Option<f32> = None;

    let flush_run = |para: &mut Option<Para>, run: &Run, html: &mut String| {
        if let Some(p) = para {
            p.html.push_str(&wrap_run(html, run));
        }
        html.clear();
    };

    xml::parse(&doc, |ev| {
        match ev {
            Ev::Start(t) => match t.name.as_str() {
                "p" => para = Some(Para::default()),
                "pPr" => in_ppr = true,
                "r" => {
                    run = Run::default();
                    run_html.clear();
                }
                "rPr" => in_run_props = true,
                "t" => in_text = true,
                "hyperlink" => {
                    if let Some(rel) = t
                        .rel("id")
                        .and_then(|id| rels.get(id))
                        .filter(|r| r.external)
                        && let Some(p) = &mut para
                    {
                        let href = escape(&rel.target);
                        if href.starts_with("http") || href.starts_with("mailto:") {
                            p.html.push_str(&format!("<a href=\"{href}\">"));
                            link_open = true;
                        }
                    }
                }
                "tbl" => out.push_str("<table>"),
                "tr" => out.push_str("<tr>"),
                "tc" => {
                    // gridSpan, tcPr içinde Empty olarak gelir ve aşağıda colspan'a çevrilir.
                    out.push_str("<td>");
                    cell_depth += 1;
                }
                "drawing" | "pict" => extent_px = None,
                "pStyle" if in_ppr => {
                    if let (Some(p), Some(level)) =
                        (&mut para, t.get("val").and_then(|id| styles.get(id)))
                    {
                        p.heading = Some(*level);
                    }
                }
                "jc" if in_ppr => {
                    if let Some(p) = &mut para {
                        p.align = match t.get("val") {
                            Some("center") => Some("center"),
                            Some("right" | "end") => Some("right"),
                            Some("both" | "distribute") => Some("justify"),
                            _ => None,
                        };
                    }
                }
                "ilvl" if in_ppr => {
                    if let Some(p) = &mut para {
                        let lvl = t.get("val").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let id = p.num.take().map(|(id, _)| id).unwrap_or_default();
                        p.num = Some((id, lvl));
                    }
                }
                "numId" if in_ppr => {
                    if let Some(p) = &mut para {
                        let lvl = p.num.take().map(|(_, l)| l).unwrap_or(0);
                        let id = t.get("val").unwrap_or("0").to_string();
                        p.num = (id != "0").then_some((id, lvl));
                    }
                }
                "b" if in_run_props => run.bold = on(&t),
                "i" if in_run_props => run.italic = on(&t),
                "u" if in_run_props => run.underline = on(&t),
                "strike" if in_run_props => run.strike = on(&t),
                "vertAlign" if in_run_props => {
                    run.sup = t.get("val") == Some("superscript");
                    run.sub = t.get("val") == Some("subscript");
                }
                "color" if in_run_props => {
                    run.color = t
                        .get("val")
                        .filter(|c| c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()))
                        .map(String::from);
                }
                "sz" if in_run_props => {
                    run.size_pt = t
                        .get("val")
                        .and_then(|v| v.parse::<f32>().ok())
                        .map(|h| h / 2.0);
                }
                "gridSpan" => {
                    if let (Some(span), true) = (t.get("val"), out.ends_with("<td>")) {
                        out.truncate(out.len() - 4);
                        out.push_str(&format!("<td colspan=\"{}\">", escape(span)));
                    }
                }
                "tab" => run_html.push_str("&emsp;"),
                "br" | "cr" => match (&mut para, t.get("type")) {
                    (Some(p), Some("page")) => p.page_break_after = true,
                    _ => run_html.push_str("<br>"),
                },
                "extent" => {
                    extent_px = t
                        .get("cx")
                        .and_then(|v| v.parse::<f32>().ok())
                        .map(|emu| emu / 9525.0);
                }
                "blip" | "imagedata" => {
                    let id = t.rel("embed").or_else(|| t.rel("id"));
                    if let (Some(p), Some(rel)) = (&mut para, id.and_then(|id| rels.get(id)))
                        && let Some(uri) = pkg.image_data_uri("word", rel)
                    {
                        let width = extent_px
                            .map(|w| format!("width:{w:.0}px;"))
                            .unwrap_or_default();
                        p.html.push_str(&format!(
                            "<img src=\"{uri}\" style=\"{width}max-width:100%;height:auto\">"
                        ));
                    }
                }
                _ => {}
            },
            Ev::Text(s) => {
                if in_text {
                    run_html.push_str(&escape(&s));
                }
            }
            Ev::End(name) => match name.as_str() {
                "t" => in_text = false,
                "rPr" => in_run_props = false,
                "pPr" => in_ppr = false,
                "r" => flush_run(&mut para, &run, &mut run_html),
                "hyperlink" => {
                    if link_open {
                        if let Some(p) = &mut para {
                            p.html.push_str("</a>");
                        }
                        link_open = false;
                    }
                }
                "p" => {
                    if let Some(p) = para.take() {
                        let in_cell = cell_depth > 0;
                        let empty = p.html.trim().is_empty();
                        let style = p
                            .align
                            .map(|a| format!(" style=\"text-align:{a}\""))
                            .unwrap_or_default();
                        if let Some(level) = p.heading.filter(|_| !empty) {
                            out.push_str(&format!("<h{level}{style}>{}</h{level}>", p.html));
                        } else if let Some((num_id, lvl)) = p.num.clone() {
                            let fmt = numbering
                                .get(&num_id)
                                .and_then(|m| m.get(&lvl))
                                .map_or("decimal", String::as_str);
                            counters.retain(|(id, l), _| id != &num_id || *l <= lvl);
                            let n = counters.entry((num_id, lvl)).or_insert(0);
                            *n += 1;
                            out.push_str(&format!(
                                "<p class=\"li\" style=\"margin-left:{:.1}em;text-indent:-1.2em;padding-left:1.2em{}\">{}&nbsp;{}</p>",
                                lvl as f32 * 1.5 + 1.2,
                                p.align.map(|a| format!(";text-align:{a}")).unwrap_or_default(),
                                marker(fmt, *n),
                                p.html
                            ));
                        } else if empty && !in_cell {
                            out.push_str("<p>&nbsp;</p>");
                        } else if !empty {
                            out.push_str(&format!("<p{style}>{}</p>", p.html));
                        }
                        if p.page_break_after {
                            out.push_str("<div class=\"pb\"></div>");
                        }
                    }
                }
                "tc" => {
                    cell_depth = cell_depth.saturating_sub(1);
                    out.push_str("</td>");
                }
                "tr" => out.push_str("</tr>"),
                "tbl" => out.push_str("</table>"),
                _ => {}
            },
        }
        true
    })?;

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(page(&title, &out, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_word_features() {
        let path = crate::pdf::fixtures::temp_file("ornek.docx");
        crate::pdf::fixtures::docx(&path);
        let html = to_html(&path).unwrap();
        // Başlık stili (yerelleştirilmiş kimlik "Balk1" -> "heading 1"), varlık çözümü
        assert!(html.contains("<h1>Başlık &amp; Özet</h1>"), "{html}");
        // Kalın, sonra b w:val="0" ile kapatılmış kalın + italik, sekme
        assert!(
            html.contains("<b>Kalın </b><i>italik</i>&emsp;sekme"),
            "{html}"
        );
        // Kendiliğinden kapanan boş paragraf
        assert!(html.contains("<p>&nbsp;</p>"));
        // Numaralı liste sayaçları ve madde işareti
        assert!(
            html.contains("1.&nbsp;Bir")
                && html.contains("2.&nbsp;İki")
                && html.contains("•&nbsp;Madde")
        );
        assert!(
            html.contains("<td colspan=\"2\"><p>Birleşik</p></td>"),
            "{html}"
        );
        assert!(
            html.contains("<a href=\"https://example.com/a?b=1&amp;c=2\">bağlantı</a>"),
            "{html}"
        );
        // Alt öğeli <a:blip r:embed> gömülür; genişlik EMU'dan (952500/9525 = 100px)
        assert!(
            html.contains("<img src=\"data:image/png;base64,") && html.contains("width:100px"),
            "{html}"
        );
        // Dış bağlantılı resim (r:link / file://) bilerek yüklenmez
        assert!(!html.contains("gizli"));
        assert_eq!(html.matches("<img").count(), 1);
    }

    #[test]
    fn not_a_docx_is_a_clear_error() {
        let path = crate::pdf::fixtures::temp_file("bozuk.docx");
        std::fs::write(&path, b"zip degil").unwrap();
        assert!(to_html(&path).unwrap_err().contains("Office"));
    }
}
