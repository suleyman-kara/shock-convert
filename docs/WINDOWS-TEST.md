# Windows'ta elle doğrulama listesi

Kayıt defteri ve Explorer davranışı Linux'ta test edilemez; bu liste o kısımlar içindir.
Çekirdek dönüştürme `cargo test` ile otomatik doğrulanır.

Hazırlık: `.\build.ps1`, ardından `dist\shock-convert-setup-*.exe` (veya `target\release\shock-convert.exe install`).
Bir klasörde farklı formatlarda birkaç deneme dosyası hazırlayın (boşluklu ad ve Türkçe karakterli ad dahil: `Fotoğraf Bir.png`).

## Menü
- [ ] `.png` dosyasına sağ tık ▸ (Win11'de "Daha fazla seçenek göster") **Shock Convert ▸** alt menüsü görünüyor.
- [ ] Alt menüde PNG **yok**, diğer 6 format var. `.jpg` dosyasında JPG yok.
- [ ] `.txt` dosyasında menü görünmüyor.
- [ ] Menüde simge (exe simgesi) görünüyor.

## Dönüştürme
- [ ] `Fotoğraf Bir.png` ▸ JPG: aynı klasörde `Fotoğraf Bir-jpg.jpg` oluştu, bildirim çıktı, siyah konsol penceresi **açılmadı**.
- [ ] Aynı işlemi tekrarlayınca `... (1).jpg` oluştu, ilki bozulmadı.
- [ ] **Çoklu seçim:** 5–15 dosya seçip aynı formata çevir. Hepsi dönüştü ve **tek** bildirim çıktı
      (Explorer her dosya için ayrı süreç başlatır; `shell-run` bunları tek toplu işe birleştirir).
      Not: klasik menüde 15'ten fazla dosya seçilirse Explorer menüyü hiç göstermeyebilir.
- [ ] Boşluklu / Türkçe karakterli dosya adları doğru işlendi (çoklu seçimde de).
- [ ] Bozuk bir `.png` (ör. metin dosyasını yeniden adlandır) ▸ hata bildirimi, yarım çıktı yok.
- [ ] Telefon fotoğrafı (EXIF yönlü) doğru dönük çıktı.

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
