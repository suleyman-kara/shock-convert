# File Converter (Tichau) incelemesi

İnceleme: suleyman-kara/shock-convert#4. İncelenen sürüm: `Tichau/FileConverter` ana dalı (commit `6c157a4`).

File Converter **GPL-3.0** lisanslıdır, Shock Convert ise **Apache-2.0**. Bu yüzden yalnızca
*davranış* incelendi; **hiçbir kod kopyalanmadı**, aşağıdakiler kendi cümlelerimizdir ve uygulanan
çözümler sıfırdan yazılmıştır.

## Mimari özeti

- Her dönüşüm bir "iş" (job) nesnesi: hazırlık (girdi/çıktı yolu doğrulama, klasör oluşturma, benzersiz
  ad üretme) → dönüştürme → başarı/başarısızlık sonrası adımlar.
- Görüntüler ImageMagick (Magick.NET), ses/video ffmpeg, Office belgeleri kurulu Microsoft Office
  (COM otomasyonu), ICO/GIF için ara PNG adımları kullanılır.
- Sağ tık menüsü bir kabuk eklentisi (shell extension) ile sağlanır; bu yüzden Explorer'ın klasik
  menüdeki 15 dosya sınırına takılmaz.

## Bulgular ve kararlar

| # | Konu | File Converter'da | Shock Convert'te | Karar |
|---|---|---|---|---|
| 1 | Başarısızlıkta yarım çıktı | Silinir | Silinir (görüntü, ffmpeg, PDF, eklenti) | ✅ Var |
| 2 | Çıktı adı çakışması | Benzersiz ad üretilir; aynı toplu işte üretilen adlar da hariç tutulur | Ad `create_new` ile atomik rezerve edilir; paralel işler ve eşzamanlı süreçler çakışamaz | ✅ Var (daha sıkı) |
| 3 | Girdi = çıktı aynı yol | Özel durum olarak ele alınır | Aynı biçime dönüştürme menüde gizli; varyant preset'lerde `(1)` eki | ✅ Var |
| 4 | Çıktı klasörü yoksa | Oluşturulur (çıktı şablonu farklı klasör seçebildiği için) | Çıktı hep kaynağın klasöründe; gerek yok | ➖ Gerek yok |
| 5 | Geçersiz çıktı yolu | Dönüşümden önce yol geçerliliği denetlenir | Çıktı adı girdiden türetilir; girdi geçerliyse çıktı da geçerli | ➖ Gerek yok |
| 6 | ffmpeg hata tespiti | stderr'deki metinden ("Error", "Exiting" …) çıkarılır; dosya adlarında "Error" geçince yanlış alarm verdiği için adlar ayıklanmak zorunda kalınmış | Çıkış kodu + boş çıktı denetimi | ✅ Bizimki daha sağlam |
| 7 | ffmpeg'e argüman geçirme | Tek bir komut satırı metni, tırnaklarla | Argüman dizisi (kabuk/tırnak sorunu yok) | ✅ Var |
| 8 | **`-` ile başlayan dosya adları** | (Explorer mutlak yol verdiği için sorun görünmez) | Komut satırında göreli `-y.mp4` verilince çıktı `-y.mp3` ffmpeg'e seçenek olarak gidiyordu | 🔧 **Düzeltildi:** girdi her zaman mutlak yola çevriliyor (ffmpeg, LibreOffice, tarayıcı, eklentiler). Test eklendi |
| 9 | **Videodan GIF kalitesi** | Önce renk paleti üretilip sonra o paletle kodlanır | Paletsiz, bantlaşmalı GIF | 🔧 **Düzeltildi:** tek geçişte `palettegen`/`paletteuse` |
| 10 | **MP3 etiket uyumluluğu** | ID3v2.3 + ID3v1 zorlanır | ffmpeg varsayılanı (ID3v2.4); Windows Gezgini eksik gösterebilir | 🔧 **Düzeltildi:** `-id3v2_version 3 -write_id3v1 1`; Türkçe etiketle doğrulandı |
| 11 | İlerleme göstergesi | ffmpeg çıktısından süre/zaman okunarak yüzde hesaplanır | Yok; yalnızca başlangıç ve bitiş bildirimi | 📋 Sonraya: `-progress pipe:1` ile yüzde, uzun videolar için |
| 12 | İptal | Kullanıcı iptal edebilir; süreç sonlandırılıp yarım çıktı silinir | İptal arayüzü yok | 📋 Sonraya (bir arayüz gerektirir) |
| 13 | Çıktı tarihini kaynakla eşitleme | Çıktının değiştirilme tarihi kaynağınkiyle aynı yapılır | Çıktı kaynağın oluşturma/değiştirme/erişim tarihlerini taşır (görüntü, ses/video, PDF, PDF araçları; eklenti çıktısına dokunulmaz) | ✅ **Eklendi** (varsayılan açık; kapatma seçeneği ayarlar arayüzüyle gelecek) |
| 14 | Dönüşüm sonrası girdiyi silme / arşiv klasörüne taşıma | Preset seçeneği | Yok | 📋 Sonraya: preset seçeneği olarak (veri kaybı riski nedeniyle varsayılan kapalı) |
| 15 | Çıktı adı/klasör şablonu | Şablon ile farklı klasör ve ad üretilebilir | Yalnızca `suffix` | 📋 Sonraya: istenirse preset'e `output_dir` |
| 16 | PDF → görüntü | Her sayfa ayrı görüntü, sabit DPI ile | Yok | 📋 #9 (PDF işlemleri) kapsamında |
| 17 | Office ile PDF | Belge salt okunur açılıp Office'in kendi PDF dışa aktarımı kullanılır | Office → LibreOffice → yerleşik sırası; Office belgeyi salt okunur, makrosuz, görünmez açar | ✅ **Eklendi** (#6). Tuzaklar aşağıda; Windows'ta Office ile test bekliyor (#17) |
| 18 | Büyük görüntüler | ImageMagick kaynak sınırlarına bağlı | `image` crate'inin varsayılan bellek sınırı aşılırsa anlaşılır hata | ✅ Var |
| 19 | Bozuk ayar dosyası | (Ayarlar uygulama içinden yönetilir) | Bozuk `presets.toml` uyarıyla atlanır, yerleşik preset'ler çalışmaya devam eder | ✅ Var |
| 20 | Kaldırıcının artık bırakması | Bilinen upstream sorunu | Kaldırma menüyü, kayıt defterini, klasörü ve günlüğü siler (kullanıcı preset'i korunur) | ✅ Var |
| 21 | Çok dosyalı seçim sınırı | Kabuk eklentisi sayesinde yok | Klasik menüde Explorer 15 dosya sınırı | 📋 #7 (kabuk eklentisi) bunu da çözer |
| 22 | Süreç çökmesinde yarım dosya | — | Ad rezervasyonu nedeniyle süreç zorla sonlandırılırsa 0 baytlık çıktı kalabilir | 📋 Sonraya: geçici adla yazıp sonra yeniden adlandırma |
| 23 | EXIF/ICC korunması | ImageMagick profilleri büyük ölçüde korur | JPG, PNG ve WebP'de EXIF (kamera, tarih, konum) ve ICC renk profili korunur; TIFF'te ICC korunur. Yön pikselere uygulandığı için EXIF yön etiketi sıfırlanır (görüntü iki kez dönmez). GIF/BMP/ICO bu verileri taşıyamaz | ✅ **Eklendi** |

## #6 (Office ile PDF) için çıkarılan dersler

- Doğru yöntem: belgeyi **salt okunur** açıp Office'in kendi **PDF olarak dışa aktar** özelliğini kullanmak.
- **Kaçınılacak tuzaklar** (File Converter'da gözlenen):
  - **PowerPoint tek örneklidir.** İş bitince uygulamayı kapatmak, kullanıcının açık sunumlarını da
    kapatabilir. Biz yalnızca açtığımız belgeyi kapatmalı, uygulamayı ancak kendimiz başlattıysak sonlandırmalıyız.
  - **Makro güvenliği ayarlanmıyor ve uyarı pencereleri bastırılmıyor.** Biz makroları otomasyon
    düzeyinde zorla kapatmalı, uyarıları bastırmalı ve parola isteyen belgelerde takılmamak için
    zaman aşımı koymalıyız.

## Doğrulama

- Birim testleri: `cargo test --workspace` (mutlak yol testi dahil).
- Gerçek ffmpeg ile:
  - `run mp3 -- -y.mp4` başarılı.
  - MP3 başlığı `ID3` v2.3, dosya sonunda ID3v1 `TAG` var, Türkçe başlık korunuyor.
  - Paletli GIF 480×360, 24 kare.
