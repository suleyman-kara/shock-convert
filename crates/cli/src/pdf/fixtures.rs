//! Testler için küçük, elle kurulmuş OOXML paketleri. Gerçek Office çıktılarındaki iki tuzağı da
//! içerir: aynı öğede `id` ve `r:id` birlikte (`<p:sldId>`), ve alt öğesi olan `<a:blip>` (boş değil).

use std::io::Write;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

/// 1x1 kırmızı PNG.
pub const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

pub fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shock-convert-fx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

pub fn build_zip(path: &Path, parts: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, data) in parts {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

pub fn docx(path: &Path) {
    let document = format!(
        r#"<?xml version="1.0"?><w:document {W}><w:body>
<w:p><w:pPr><w:pStyle w:val="Balk1"/></w:pPr><w:r><w:t>Başlık &amp; Özet</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Kalın </w:t></w:r><w:r><w:rPr><w:b w:val="0"/><w:i/></w:rPr><w:t>italik</w:t></w:r><w:r><w:tab/><w:t>sekme</w:t></w:r></w:p>
<w:p/>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Bir</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>İki</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr><w:r><w:t>Madde</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>Birleşik</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:hyperlink r:id="rIdLink"><w:r><w:t>bağlantı</w:t></w:r></w:hyperlink></w:p>
<w:p><w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="952500"/><a:graphic><a:graphicData><a:blip r:embed="rIdImg"><a:extLst/></a:blip></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
<w:p><w:r><w:drawing><a:blip r:link="rIdExt"/></w:drawing></w:r></w:p>
</w:body></w:document>"#
    );
    let styles = format!(
        r#"<w:styles {W}><w:style w:styleId="Balk1"><w:name w:val="heading 1"/></w:style></w:styles>"#
    );
    let numbering = format!(
        r#"<w:numbering {W}><w:abstractNum w:abstractNumId="10"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="11"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="10"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="11"/></w:num></w:numbering>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Target="media/i.png"/><Relationship Id="rIdLink" Target="https://example.com/a?b=1&amp;c=2" TargetMode="External"/><Relationship Id="rIdExt" Target="file:///C:/gizli.png" TargetMode="External"/></Relationships>"#;
    build_zip(
        path,
        &[
            ("word/document.xml", document.as_bytes()),
            ("word/styles.xml", styles.as_bytes()),
            ("word/numbering.xml", numbering.as_bytes()),
            ("word/_rels/document.xml.rels", rels.as_bytes()),
            ("word/media/i.png", PNG_1X1),
        ],
    );
}

const P: &str = r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

pub fn pptx(path: &Path) {
    let presentation = format!(
        r#"<p:presentation {P}><p:sldIdLst><p:sldId id="256" r:id="rId7"/><p:sldId id="257" r:id="rId8"/></p:sldIdLst></p:presentation>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId7" Target="slides/slide2.xml"/><Relationship Id="rId8" Target="slides/slide1.xml"/></Relationships>"#;
    let s_first = format!(
        r#"<p:sld {P}><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"><p:extLst/></p:ph></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>İlk slayt</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:pPr lvl="1"/><a:r><a:t>girintili</a:t></a:r></a:p></p:txBody></p:sp><p:pic><p:blipFill><a:blip r:embed="rId1"><a:extLst/></a:blip></p:blipFill></p:pic></p:spTree></p:cSld></p:sld>"#
    );
    let s_second = format!(
        r#"<p:sld {P}><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>İkinci</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
    );
    let slide_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Target="../media/x.png"/></Relationships>"#;
    build_zip(
        path,
        &[
            ("ppt/presentation.xml", presentation.as_bytes()),
            ("ppt/_rels/presentation.xml.rels", rels.as_bytes()),
            // Sıra presentation.xml'deki sldIdLst'ten gelir, dosya adından değil.
            ("ppt/slides/slide2.xml", s_first.as_bytes()),
            ("ppt/slides/_rels/slide2.xml.rels", slide_rels.as_bytes()),
            ("ppt/slides/slide1.xml", s_second.as_bytes()),
            ("ppt/media/x.png", PNG_1X1),
        ],
    );
}

pub fn xlsx(path: &Path) {
    let ns = r#"xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
    let workbook = format!(
        r#"<workbook {ns}><sheets><sheet name="Satış" sheetId="1" r:id="rId1"/><sheet name="Gizli" sheetId="2" state="hidden" r:id="rId2"/></sheets></workbook>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="/xl/worksheets/sheet2.xml"/></Relationships>"#;
    let shared = format!(
        r#"<sst {ns}><si><t>Şehir</t></si><si><r><t>İst</t></r><r><t>anbul</t></r><rPh><t>X</t></rPh></si></sst>"#
    );
    let styles = format!(
        r#"<styleSheet {ns}><numFmts><numFmt numFmtId="164" formatCode="dd.mm.yyyy"/></numFmts><cellXfs><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="14"/></cellXfs></styleSheet>"#
    );
    let sheet1 = format!(
        r#"<worksheet {ns}><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="inlineStr"><is><t>Not &lt;1&gt;</t></is></c></row><row r="3"><c r="A3" t="s"><v>1</v></c><c r="B3"><v>1250.5</v></c><c r="C3" s="1"><v>45000</v></c><c r="D3" t="b"><v>1</v></c><c r="E3" s="2"/></row></sheetData></worksheet>"#
    );
    let sheet2 = format!(
        r#"<worksheet {ns}><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>GİZLİ</t></is></c></row></sheetData></worksheet>"#
    );
    build_zip(
        path,
        &[
            ("xl/workbook.xml", workbook.as_bytes()),
            ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
            ("xl/sharedStrings.xml", shared.as_bytes()),
            ("xl/styles.xml", styles.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet1.as_bytes()),
            ("xl/worksheets/sheet2.xml", sheet2.as_bytes()),
        ],
    );
}
