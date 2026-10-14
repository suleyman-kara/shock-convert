# PDF işlemleri: araç seçimi

İş: suleyman-kara/shock-convert#9. Shock Convert Apache-2.0 lisanslıdır; kod olarak kullanılacak
kütüphanelerin buna uyumlu olması gerekir (MIT/BSD/Apache uygun, GPL/AGPL uygun değil).

| Araç | Lisans | Değerlendirme |
|---|---|---|
| **lopdf** (Rust) | MIT | **Seçildi.** PDF nesne modelini okuyup yazar; birleştirme, bölme, döndürme için yeterli ve süreç içi çalışır |
| PDFium (+ pdfium-render) | BSD / Apache-2.0 | Sayfayı görüntüye çizme gerektiren işler için aday (PDF → PNG/JPG, görüntüleri yeniden örnekleyerek sıkıştırma). DLL paketlemesi gerektirir |
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

## Sonraya kalanlar
- **PDF → görüntü** ve **sıkıştırma:** sayfa çizen bir motor (PDFium) gerektirir.
- **Sayfa seç/çıkar** ve **parola koy/kaldır:** kullanıcıdan girdi alan küçük bir pencere gerektirir.
- **Metin çıkarma:** lopdf'in metin çıkarma desteği karmaşık yazı tiplerinde güvenilir değil. Daha iyi bir çözümle birlikte ele alınmalı.
