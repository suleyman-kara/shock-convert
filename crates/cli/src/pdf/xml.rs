//! quick-xml üzerine ince bir sarmalayıcı: yalnızca yerel adlar (ad alanı öneki yok) ve
//! çözülmüş öznitelik/metin değerleri. Belge çözümleyicileri bunun üzerinden yazılır.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

#[derive(Debug, Clone, Default)]
pub struct Tag {
    /// Yerel ad: `w:p` -> `p`.
    pub name: String,
    /// (yerel ad, önekli mi, değer)
    attrs: Vec<(String, bool, String)>,
}

impl Tag {
    /// Özniteliği yerel adıyla alır: `w:val` / `r:id` -> `val` / `id`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, _, v)| v.as_str())
    }

    /// İlişki kimliği gibi önekli (ad alanlı) öznitelik: `<p:sldId id="256" r:id="rId2">` içinde
    /// `rel("id")` -> `rId2`. Önekin harfi belgeden belgeye değişebileceği için yalnızca önekli
    /// olup olmadığına bakılır.
    pub fn rel(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, prefixed, _)| k == key && *prefixed)
            .map(|(_, _, v)| v.as_str())
    }
}

#[derive(Debug)]
/// Boş etiketler (`<a/>`) `Start` + `End` olarak yayılır; böylece çözümleyiciler bir öğenin
/// alt öğesi olup olmamasından bağımsız olarak özniteliklerini hep `Start`'ta okur.
pub enum Ev {
    Start(Tag),
    End(String),
    Text(String),
}

fn tag_of(e: &BytesStart) -> Tag {
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            let key = a.key.local_name().as_ref().to_string();
            let prefixed = a.key.prefix().is_some();
            let value = a
                .normalized_value(XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .unwrap_or_default();
            (key, prefixed, value)
        })
        .collect();
    Tag {
        name: e.local_name().as_ref().to_string(),
        attrs,
    }
}

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => return None,
    })
}

/// Belgeyi olay olay gezer; `f` `false` dönerse durur (büyük dosyalarda erken çıkış için).
/// Bozuk XML'de o ana kadar okunanlar işlenmiş olur ve hata döner.
pub fn parse(xml: &str, mut f: impl FnMut(Ev) -> bool) -> Result<(), String> {
    let mut reader = Reader::from_str(xml);
    loop {
        let keep_going = match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => f(Ev::Start(tag_of(&e))),
            Event::Empty(e) => {
                let tag = tag_of(&e);
                let name = tag.name.clone();
                f(Ev::Start(tag)) && f(Ev::End(name))
            }
            Event::End(e) => f(Ev::End(e.local_name().as_ref().to_string())),
            Event::Text(t) => f(Ev::Text(t.xml10_content().into_owned())),
            Event::CData(c) => f(Ev::Text(c.into_inner().into_owned())),
            Event::GeneralRef(r) => {
                let ch = if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten()
                } else {
                    entity(&r)
                };
                match ch {
                    Some(c) => f(Ev::Text(c.to_string())),
                    None => true,
                }
            }
            Event::Eof => return Ok(()),
            _ => true,
        };
        if !keep_going {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationship_id_is_not_confused_with_plain_id() {
        let mut tag = None;
        parse(r#"<p:sldId id="256" r:id="rId2"/>"#, |e| {
            if let Ev::Start(t) = e {
                tag = Some(t);
            }
            true
        })
        .unwrap();
        let t = tag.unwrap();
        assert_eq!(t.get("id"), Some("256"));
        assert_eq!(t.rel("id"), Some("rId2"));
    }

    #[test]
    fn local_names_entities_and_attributes() {
        let mut seen = Vec::new();
        parse(
            r#"<w:p w:val="a&amp;b"><w:t>Tom &amp; Jerry &#x15F; &lt;3</w:t><w:br/></w:p>"#,
            |e| {
                seen.push(match e {
                    Ev::Start(t) => format!("S:{}:{}", t.name, t.get("val").unwrap_or("")),
                    Ev::End(n) => format!("X:{n}"),
                    Ev::Text(s) => format!("T:{s}"),
                });
                true
            },
        )
        .unwrap();
        let joined = seen.join("|");
        assert!(joined.starts_with("S:p:a&b|S:t:"), "{joined}");
        let text: String = seen.iter().filter_map(|s| s.strip_prefix("T:")).collect();
        assert_eq!(text, "Tom & Jerry ş <3");
        assert!(joined.contains("S:br:|X:br") && joined.ends_with("X:p"));
    }
}
