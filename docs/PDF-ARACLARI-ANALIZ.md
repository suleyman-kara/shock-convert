# PDF işlemleri: araç seçimi

İş: suleyman-kara/shock-convert#9. Shock Convert Apache-2.0 lisanslıdır; kod olarak kullanılacak
kütüphanelerin buna uyumlu olması gerekir (MIT/BSD/Apache uygun, GPL/AGPL uygun değil).

| Araç | Lisans | Değerlendirme |
|---|---|---|
| **lopdf** (Rust) | MIT | **Seçildi.** PDF nesne modelini okuyup yazar; birleştirme, bölme, döndürme için yeterli ve süreç içi çalışır |
| PDFium (+ pdfium-render) | BSD / Apache-2.0 (pdfium-render: MIT/Apache-2.0) | **Seçildi** (PDF → görüntü, PDF → metin). Hazır derleme [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries); sürüm ve SHA-256 `third_party/pdfium.txt` içinde sabit, `pdfium.dll` kurulumla gelir (~7 MB) |
| qpdf | Apache-2.0 | Şifreleme/çözme, doğrusallaştırma için referans; gerekirse harici araç |
| pdfcpu (Go) | Apache-2.0 | Geniş işlem kümesi; özellik ve kenar durumları için referans |
| Ghostscript, MuPDF, PDFsam Basic | AGPL | Kodlarına gömülemez; yalnızca davranış referansı |

Not: Bu seçim lisans ve yetenek temelinde yapıldı. Araçların kaynak kodları ayrıntılı incelenmedi.

## İlk sürümde yapılanlar
- **PDF: Birleştir:** Birden çok PDF ve görüntü seçilir, tek PDF çıkar. Sıra dosya adına göre doğal sıradır (2, 10'dan önce gelir). Çıktı ilk dosyanın klasörüne `<ilk-ad>-birleşik.pdf` olarak yazılır.
- **PDF: Sayfalara böl:** Her sayfa ayrı bir dosyaya yazılır (`<ad>-sayfa-01.pdf` …).
- **PDF: 90° sağa / 90° sola / 180° döndür:** Tüm sayfalar döner; yer imleri ve formlar korunur.

Uygulama ayrıntıları:
- **Sayfa ağacı:** Birleştirme ve bölme, seçilen sayfalardan sıfırdan temiz bir sayfa ağacı kurar. Bu sırada sayfaların üst düğümlerden miras aldığı özellikler (MediaBox, CropBox, Resources, Rotate) sayfanın kendisine yazılır.
- **Neden kendi kodumuz:** lopdf'in `delete_pages` işlevi, silinen sayfanın bağlantısını üst düğümün `Kids` listesinde bıraktığı için kullanılmadı.
- **Şifreli ve parola korumalı PDF'ler reddedilir.** Yeni belge kurmak, sahibinin koyduğu kısıtlamaları sessizce kaldırırdı.
- **Birleştirmede korunmayanlar:** yer imleri, form alanları ve belge bilgisi.

## İkinci sürümde yapılanlar
- **PDF: Küçült** (lopdf, süreç içi): 8 bit gri/RGB resimler (JPEG ya da Flate) çözülür, en uzun kenarı `max_width` (varsayılan 1600 px) üstündeyse küçültülür ve `quality` (varsayılan 70) ile JPEG olarak yeniden kodlanır. Yeni hali en az %10 küçük değilse eskisi kalır. Renk anahtarı maskeli, özel `Decode` dizili, CMYK, indeksli ve 1 bit resimlere dokunulmaz (renk bozulmasın). Ardından akışlar sıkıştırılır, kullanılmayan nesneler atılır, nesne akışları ve xref akışıyla (PDF 1.5) kaydedilir. Sonuç kaynaktan küçük değilse dosya yazılmaz.
- **PDF: Görüntülere çevir (PNG/JPG)** (PDFium): sayfa beyaz zemine `dpi` (varsayılan 150) çözünürlükte, form alanları ve açıklamalarla birlikte çizilir. Bir kenar en fazla 12 000 px.
- **PDF: Metne çevir (TXT)** (PDFium): sayfaların metni okuma sırasıyla, sayfalar boş satırla ayrılarak UTF-8 yazılır. Metin yoksa (taranmış belge) anlaşılır hata.
- Testler: PDFium varsa (CI'da indirilir) gerçek çizim ve metin çıkarma; Windows CI'da `pdfium.dll`'in yüklendiği PNG → PDF → PNG ile denenir.

## Sonraya kalanlar
- **Sayfa seç/çıkar** ve **parola koy/kaldır:** kullanıcıdan girdi alan küçük bir pencere gerektirir (ayarlar arayüzüyle birlikte, #18).
- **Metin tanıma (OCR)** taranmış PDF'ler için: kapsam dışı.
