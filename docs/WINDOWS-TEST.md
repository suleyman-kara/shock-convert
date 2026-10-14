# Windows'ta elle doğrulama listesi

Kayıt defteri ve Explorer davranışı Linux'ta test edilemez; bu liste o kısımlar içindir.
Çekirdek dönüştürme `cargo test` ile otomatik doğrulanır.

Hazırlık: `.\build.ps1`, ardından `dist\shock-convert-setup-*.exe` (veya `target\release\shock-convert.exe install`).
Bir klasörde farklı formatlarda birkaç deneme dosyası hazırlayın (boşluklu ad ve Türkçe karakterli ad dahil: `Fotoğraf Bir.png`).

## Menü
- [ ] Shock Convert alt menüsünün solunda şimşek ikonu görünüyor; `shock-convert.exe` dosyasının da ikonu var.
- [ ] `.png` dosyasına sağ tık ▸ (Win11'de "Daha fazla seçenek göster") **Shock Convert ▸** alt menüsü görünüyor.
- [ ] Alt menüde PNG **yok**, diğer 6 format var. `.jpg` dosyasında JPG yok.
- [ ] `.txt` dosyasında menü görünmüyor.
- [ ] Menüde simge (exe simgesi) görünüyor.

## Dönüştürme
- [ ] `Fotoğraf Bir.png` ▸ JPG: aynı klasörde `Fotoğraf Bir.jpg` oluştu, bildirim çıktı, siyah konsol penceresi **açılmadı**.
- [ ] Aynı işlemi tekrarlayınca `... (1).jpg` oluştu, ilki bozulmadı.
- [ ] **Çoklu seçim:** 5–15 dosya seçip aynı formata çevir. Hepsi dönüştü ve **tek** bildirim çıktı
      (Explorer her dosya için ayrı süreç başlatır; `shell-run` bunları tek toplu işe birleştirir).
      Not: klasik menüde 15'ten fazla dosya seçilirse Explorer menüyü hiç göstermeyebilir.
- [ ] Boşluklu / Türkçe karakterli dosya adları doğru işlendi (çoklu seçimde de).
- [ ] Bozuk bir `.png` (ör. metin dosyasını yeniden adlandır) ▸ hata bildirimi, yarım çıktı yok.
- [ ] Telefon fotoğrafı (EXIF yönlü) doğru dönük çıktı.

## PDF
- [ ] `.png` ve `.jpg` ▸ PDF: anında `ad.pdf` oluştu, görüntü tam sayfa.
- [ ] `.txt` (Türkçe karakterli; ayrıca Not Defteri'nde ANSI kaydedilmiş bir dosya) ▸ PDF: karakterler doğru.
- [ ] `.md` ve `.html` ▸ PDF çalışıyor ("dönüştürülüyor…" bildirimi, sonra tamamlandı).
- [ ] Office ile PDF (Office kurulu bir bilgisayarda): ayrıntılı test listesi #17'de.
- [ ] Office ve LibreOffice **kurulu değilken** gerçek bir `.docx` (başlık, liste, tablo, resim içeren), `.xlsx`, `.pptx` ▸ PDF: içerik okunaklı.
- [ ] Office ve LibreOffice kurulu değilken `.doc` / `.odt` ▸ PDF: bildirimde `winget install TheDocumentFoundation.LibreOffice` yönlendirmesi.
- [ ] `winget install TheDocumentFoundation.LibreOffice` sonrası aynı `.docx` ▸ PDF: LibreOffice çıktısı (düzen Word'e daha yakın); `.doc`/`.odt` de çalışıyor.
- [ ] Edge açıkken de PDF dönüştürme çalışıyor (ayrı profil kullanılır).

## PDF işlemleri
- [ ] 3–4 PDF ve bir görüntü seçip ▸ PDF: Birleştir: tek bildirim, tek `<ilk-ad>-birleşik.pdf`, sayfa sırası dosya adlarının doğal sırası.
- [ ] Çok sayfalı PDF ▸ PDF: Sayfalara böl: her sayfa ayrı dosya, Edge'de doğru açılıyor.
- [ ] PDF ▸ 90° sağa döndür: sayfalar dönmüş, orijinal dosya değişmemiş.
- [ ] Parola korumalı bir PDF'te anlaşılır hata bildirimi çıkıyor ve yarım dosya kalmıyor.
- [ ] Taranmış/fotoğraflı büyük bir PDF ▸ PDF: Küçült: `<ad>-küçük.pdf` belirgin şekilde küçük, Edge'de düzgün açılıyor, metin net.
- [ ] Zaten küçük bir PDF ▸ PDF: Küçült: "küçültülemedi" bildirimi, dosya oluşmadı.
- [ ] Çok sayfalı PDF ▸ PDF: Görüntülere çevir (PNG) ve (JPG): her sayfa ayrı görüntü, yazılar okunaklı; tek sayfalı PDF'te `ad.png`.
- [ ] Türkçe karakterli bir PDF ▸ PDF: Metne çevir (TXT): Not Defteri'nde karakterler doğru; taranmış PDF'te "metin yok" bildirimi.
- [ ] Kurulum klasöründe `pdfium.dll` ve `licenses\pdfium` var; kaldırınca hepsi siliniyor.

## Ses / video / preset
- [ ] ffmpeg kurulu değilken bir `.mp4` ▸ MP3: bildirimde `winget install Gyan.FFmpeg` yönlendirmesi çıkıyor.
- [ ] `winget install Gyan.FFmpeg` sonrası (yeni terminal/oturum) `.mp4` ▸ MP3 ve `.mp4` ▸ WebM çalışıyor; başta "dönüştürülüyor…" bildirimi, sonda tamamlandı bildirimi var.
- [ ] `.mp3` dosyasında video preset'leri (MP4/WebM/GIF) görünmüyor; `.mp4`'te MP3/WAV/FLAC/OGG/M4A görünüyor.
- [ ] `shock-convert presets` dosya yolunu yazdı; dosyaya örnekteki `web-jpg` preset'ini açıp `shock-convert register` sonrası menüde göründü ve `foto-web.jpg` üretti.
- [ ] Bozuk bir `.mp4` (metin dosyasını yeniden adlandır) ▸ MP3: hata bildirimi, çıktı dosyası kalmadı.

## Kurulum / kaldırma
- [ ] Kurulum sonrası yönetici sorusu çıkmadı; Ayarlar ▸ Uygulamalar'da "Shock Convert" var.
- [ ] Explorer'ı yeniden başlatmadan menü görünüyor.
- [ ] Kaldırınca `%LOCALAPPDATA%\ShockConvert` (günlük) da silindi; `%APPDATA%\ShockConvert\presets.toml` (kullanıcı verisi) kaldı.
- [ ] Kaldırınca: menü kayboldu; `%LOCALAPPDATA%\Programs\ShockConvert` silindi;
      `reg query "HKCU\Software\Classes\SystemFileAssociations\.png"` anahtar bulamıyor;
      `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert` bulamıyor.
- [ ] `shock-convert.exe install` / `uninstall` (sihirbazsız yol) için aynı kontroller.

## Terminal
- [ ] `shock-convert list` ve `shock-convert run jpg dosya.png` terminalde çıktı yazdırıyor (GUI alt sistemi + konsola bağlanma).
