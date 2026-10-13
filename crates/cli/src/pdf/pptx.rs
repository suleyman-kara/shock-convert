//! PPTX -> HTML (Office olmadan). Her slayt bir yatay sayfa olur: başlık, metin kutuları (madde
//! işaretleri ve girinti), tablolar ve resimler. Kapsam dışı: konum/düzen, şekiller, grafikler,
//! animasyonlar, konuşmacı notları, arka planlar. Düzen doğruluğu için LibreOffice önerilir.

use std::path::Path;

use super::html::{escape, page};
use super::ooxml::{Package, resolve};
use super::xml::{self, Ev};

#[derive(Default)]
struct ParaState {
    level: usize,
    bullet: bool,
    html: String,
}

fn render_slide(
    pkg: &mut Package,
    slide_xml: &str,
    slide_part: &str,
    number: usize,
    out: &mut String,
) {
    let (dir, _) = slide_part.rsplit_once('/').unwrap_or(("", slide_part));
    let rels = pkg.rels_of(slide_part);

    let mut body = String::new();
    let mut title = String::new();
    let (mut in_sp, mut is_title, mut in_tbl, mut in_tc) = (false, false, false, false);
    let mut para: Option<ParaState> = None;
    let mut in_t = false;
    let mut shape_html = String::new();

    let _ = xml::parse(slide_xml, |ev| {
        match ev {
            Ev::Start(t) => match t.name.as_str() {
                "sp" => {
                    in_sp = true;
                    is_title = false;
                    shape_html.clear();
                }
                "tbl" => {
                    in_tbl = true;
                    body.push_str("<table>");
                }
                "tr" if in_tbl => body.push_str("<tr>"),
                "tc" if in_tbl => {
                    in_tc = true;
                    body.push_str("<td>");
                }
                "p" => para = Some(ParaState::default()),
                "pPr" => {
                    if let Some(p) = &mut para {
                        p.level = t.get("lvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                "t" => in_t = true,
                "ph" if in_sp => is_title = matches!(t.get("type"), Some("title" | "ctrTitle")),
                "buChar" | "buAutoNum" => {
                    if let Some(p) = &mut para {
                        p.bullet = true;
                    }
                }
                "br" => {
                    if let Some(p) = &mut para {
                        p.html.push_str("<br>");
                    }
                }
                "blip" => {
                    if let Some(rel) = t.rel("embed").and_then(|id| rels.get(id))
                        && let Some(uri) = pkg.image_data_uri(dir, rel)
                    {
                        body.push_str(&format!(
                            "<p><img src=\"{uri}\" style=\"max-height:70vh\"></p>"
                        ));
                    }
                }
                _ => {}
            },
            Ev::Text(s) => {
                if let (true, Some(p)) = (in_t, &mut para) {
                    p.html.push_str(&escape(&s));
                }
            }
            Ev::End(name) => match name.as_str() {
                "t" => in_t = false,
                "p" => {
                    if let Some(p) = para.take() {
                        if in_tc {
                            body.push_str(&p.html);
                            body.push_str("<br>");
                        } else if !p.html.trim().is_empty() {
                            if is_title && title.is_empty() {
                                title = p.html;
                            } else {
                                let marker = if p.bullet { "•&nbsp;" } else { "" };
                                shape_html.push_str(&format!(
                                    "<p class=\"li\" style=\"margin-left:{:.1}em\">{marker}{}</p>",
                                    p.level as f32 * 1.5,
                                    p.html
                                ));
                            }
                        }
                    }
                }
                "sp" => {
                    in_sp = false;
                    body.push_str(&shape_html);
                    shape_html.clear();
                }
                "tc" if in_tbl => {
                    in_tc = false;
                    body.push_str("</td>");
                }
                "tr" if in_tbl => body.push_str("</tr>"),
                "tbl" => {
                    in_tbl = false;
                    body.push_str("</table>");
                }
                _ => {}
            },
        }
        true
    });

    let heading = if title.is_empty() {
        format!("Slayt {number}")
    } else {
        title
    };
    out.push_str(&format!(
        "<section class=\"slide\"><h2>{heading}</h2>{body}<p style=\"color:#888;font-size:9pt\">{number}</p></section>"
    ));
}

pub fn to_html(path: &Path) -> Result<String, String> {
    let mut pkg = Package::open(path)?;
    let presentation = pkg
        .read_string("ppt/presentation.xml")
        .ok_or("ppt/presentation.xml yok: geçerli bir PPTX değil")?;
    let rels = pkg.rels_of("ppt/presentation.xml");

    let mut slides: Vec<String> = Vec::new();
    let _ = xml::parse(&presentation, |ev| {
        if let Ev::Start(t) = ev
            && t.name == "sldId"
            && let Some(rel) = t.rel("id").and_then(|id| rels.get(id))
        {
            slides.push(resolve("ppt", &rel.target));
        }
        true
    });
    if slides.is_empty() {
        return Err("slayt bulunamadı".into());
    }

    let mut body = String::new();
    for (i, part) in slides.iter().enumerate() {
        if let Some(xml_text) = pkg.read_string(part) {
            render_slide(&mut pkg, &xml_text, part, i + 1, &mut body);
        }
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
    fn slides_follow_presentation_order_with_titles_indent_and_images() {
        let path = crate::pdf::fixtures::temp_file("sunum.pptx");
        crate::pdf::fixtures::pptx(&path);
        let html = to_html(&path).unwrap();
        // sldId'deki düz id="256" ile r:id karışmamalı; sıra dosya adından değil listeden gelir
        let first = html.find("İlk slayt").expect("ilk slayt yok");
        let second = html.find("İkinci").expect("ikinci slayt yok");
        assert!(first < second);
        assert!(html.contains("<h2>İlk slayt</h2>"), "{html}");
        // Kendiliğinden kapanan <a:pPr lvl="1"/> girintisi okunur
        assert!(html.contains("margin-left:1.5em\">girintili"), "{html}");
        // Alt öğeli blip
        assert!(html.contains("data:image/png;base64,"));
        assert!(
            html.contains("<h2>Slayt 2</h2>"),
            "başlıksız slayt numarayla adlanır: {html}"
        );
    }
}
