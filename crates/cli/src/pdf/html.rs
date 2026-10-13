//! Tarayıcıya basılacak HTML sayfası için ortak şablon ve yardımcılar.

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

const BASE_CSS: &str = r#"
body { font-family: "Segoe UI", "Noto Sans", Arial, Helvetica, sans-serif; font-size: 11pt; line-height: 1.35; color: #111; }
h1, h2, h3, h4, h5, h6 { line-height: 1.2; margin: 0.9em 0 0.35em; }
p { margin: 0 0 0.55em; }
p.li { margin: 0 0 0.2em; }
table { border-collapse: collapse; margin: 0.4em 0 0.8em; }
td, th { border: 1px solid #b5b5b5; padding: 3px 6px; vertical-align: top; }
td.n { text-align: right; }
img { max-width: 100%; height: auto; }
pre { white-space: pre-wrap; word-wrap: break-word; font-family: Consolas, "DejaVu Sans Mono", monospace; font-size: 10pt; }
code { font-family: Consolas, "DejaVu Sans Mono", monospace; }
blockquote { margin: 0.5em 0 0.5em 1em; padding-left: 0.8em; border-left: 3px solid #ccc; color: #444; }
.pb { break-after: page; }
section.slide { break-after: page; min-height: 90vh; }
section.slide:last-child, section.sheet:last-child { break-after: auto; }
section.sheet { break-after: page; }
"#;

/// Tam HTML belgesi. `landscape` yatay A4 sayfa ister (tablolar ve slaytlar için).
pub fn page(title: &str, body: &str, landscape: bool) -> String {
    let size = if landscape { "A4 landscape" } else { "A4" };
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title><style>@page {{ size: {size}; margin: 16mm; }}{BASE_CSS}</style></head><body>{body}</body></html>",
        escape(title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_and_builds_page() {
        assert_eq!(
            escape("<a href=\"x\">&"),
            "&lt;a href=&quot;x&quot;&gt;&amp;"
        );
        let p = page("A<B", "<p>x</p>", true);
        assert!(
            p.contains("<title>A&lt;B</title>")
                && p.contains("A4 landscape")
                && p.contains("<p>x</p>")
        );
    }
}
