//! Düz metin ve Markdown -> HTML.

use pulldown_cmark::{Options, Parser, html};

use super::html::{escape, page};

/// BOM'a bakar, geçerli UTF-8 ise onu kullanır, değilse eski Türkçe dosyalar için Windows-1254 varsayar.
pub fn decode(bytes: &[u8]) -> String {
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc
            .decode_without_bom_handling(&bytes[bom_len..])
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1254
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

pub fn plain_to_html(title: &str, bytes: &[u8]) -> String {
    page(
        title,
        &format!("<pre>{}</pre>", escape(&decode(bytes))),
        false,
    )
}

pub fn markdown_to_html(title: &str, bytes: &[u8]) -> String {
    let md = decode(bytes);
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut body = String::new();
    html::push_html(&mut body, Parser::new_ext(&md, opts));
    page(title, &body, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_bom_utf16_and_legacy_turkish() {
        assert_eq!(decode("şğıİ".as_bytes()), "şğıİ");
        assert_eq!(decode(b"\xEF\xBB\xBFmerhaba"), "merhaba");
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("öç".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        assert_eq!(decode(&utf16), "öç");
        assert_eq!(decode(b"\xFD\xFE\xF0"), "ışğ"); // Windows-1254: ý->ı, þ->ş, ð->ğ
    }

    #[test]
    fn plain_text_is_escaped_and_markdown_renders() {
        assert!(plain_to_html("t", b"<b>x</b>").contains("&lt;b&gt;x&lt;/b&gt;"));
        let h = markdown_to_html("t", b"# Baslik\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(h.contains("<h1>Baslik</h1>") && h.contains("<table>"));
    }
}
