# Shock Convert

Windows'ta görüntü, ses, video ve belge dosyalarına **sağ tıklayıp** istediğiniz formata çeviren küçük ve hızlı bir araç.

**Web sitesi ve indirme:** https://suleyman-kara.github.io/shock-convert/

```
foto.png     ──sağ tık──▶  Shock Convert ▸ JPG  ──▶  foto.jpg     (aynı klasörde)
rapor.docx   ──sağ tık──▶  Shock Convert ▸ PDF  ──▶  rapor.pdf
```

- Görüntü: **PNG, JPG, WebP, BMP, GIF, TIFF, ICO** (girdi olarak `jpeg`, `jfif`, `tif` uzantıları da tanınır). Süreç içinde, harici program başlatmadan dönüştürülür.
- Ses/video: **MP3, WAV, FLAC, OGG, M4A, MP4, WebM, GIF (videodan)**. Bunlar için [ffmpeg](https://ffmpeg.org) gerekir (aşağıya bakın).
- **PDF**: görüntüler, TXT/Markdown/HTML ve Word/Excel/PowerPoint belgeleri. **Office gerekmez** (aşağıya bakın).
- Kendi **preset**'lerinizi `presets.toml` ile ekleyin: "web için 1920px JPG", "720p MP4" gibi.
- Birden fazla dosya seçilebilir; hepsi tek seferde, paralel işlenir ve tek bildirim gösterilir.
- Çıktı kaynağın yanına **aynı adla** yazılır: `foto.png` → `foto.jpg`. Ad doluysa `foto (1).jpg` olur, **mevcut dosyanın üzerine asla yazılmaz**.
- Yönetici hakkı gerekmez (her şey kullanıcı profiline kurulur). Windows 10 ve Windows 11'in klasik menüsünde ("Daha fazla seçenek göster") görünür.
- Tek bir küçük `.exe`; Python veya başka bir çalışma zamanı gerekmez.

## Kurulum / Kaldırma

**Kurulum sihirbazı ile:** `shock-convert-setup-<sürüm>.exe` dosyasını çalıştırın. Kaldırma için *Ayarlar ▸ Uygulamalar* listesinden "Shock Convert" seçilir; menü, kayıt defteri anahtarları ve dosyalar tamamen silinir.

**Sihirbazsız:** `shock-convert.exe install` kendini `%LOCALAPPDATA%\Programs\ShockConvert` altına kopyalar, menüyü kaydeder ve kaldırma girdisini ekler. `shock-convert.exe uninstall` hepsini geri alır.

> Menü, exe'nin bulunduğu yolu gösterir. Bu yüzden exe'yi kurulumdan sonra taşımayın; taşırsanız tekrar `install` çalıştırın.

## ffmpeg (ses/video için)

ffmpeg paketlenmez. Şu sırayla aranır: `presets.toml` içindeki `ffmpeg_path`, kurulum klasöründeki `ffmpeg\ffmpeg.exe`, `PATH`.
En kolay kurulum: `winget install Gyan.FFmpeg`. Bulunamazsa bildirimde bu komut gösterilir. Görüntü dönüştürme ffmpeg gerektirmez.

## PDF (Office olmadan)

| Girdi | Nasıl dönüştürülür | Gereken |
|---|---|---|
| PNG, JPG, WebP, BMP, GIF, TIFF, ICO | Kendi PDF yazıcımız. JPG'ler yeniden sıkıştırılmadan gömülür | Hiçbir şey |
| TXT, MD, HTML | Tarayıcı motoruyla "PDF olarak yazdır" | Edge (Windows 10/11'de zaten var) veya Chrome |
| DOCX, XLSX, PPTX | Yerleşik dönüştürücü belgeyi HTML'e çevirir, tarayıcı PDF'e basar. **LibreOffice kuruluysa otomatik olarak o kullanılır** | Hiçbir şey (LibreOffice isteğe bağlı) |
| DOC, XLS, PPT, RTF, ODT, ODS, ODP | LibreOffice | `winget install TheDocumentFoundation.LibreOffice` |

**Yerleşik dönüştürücünün kapsamı.** Word: başlıklar, kalın/italik/altı çizili, renk ve yazı boyutu,
hizalama, madde ve numaralı listeler, tablolar, köprüler, gömülü resimler, sayfa sonları. Excel: görünür
sayfalar tablo olarak, tarih hücreleri tarihe çevrilir. PowerPoint: her slayt bir sayfa; başlık, maddeler, tablolar, resimler.
Desteklenmeyenler: üst/alt bilgi, dipnot, metin kutuları, şekiller, grafikler, Excel sayı biçimleri (para birimi vb.),
slayt yerleşimi ve arka planları, birebir sayfa düzeni. Bu tür belgelerde LibreOffice kurmanız önerilir.
Belgelerdeki **dış bağlantılı** resimler (internetten ya da diskteki başka bir dosyadan) güvenlik için yüklenmez.

Motor `presets.toml` içinde seçilebilir: `pdf_engine = "auto"` (varsayılan), `"builtin"` veya `"libreoffice"`.
Ham HTML dosyalarında sayfa boyutunu HTML'in kendisi belirler.

## Preset'ler

`shock-convert presets` dosya yolunu yazar ve yoksa açıklamalı bir örnek oluşturur (`%APPDATA%\ShockConvert\presets.toml`).
Düzenledikten sonra `shock-convert register` ile menüyü yenileyin. Yerleşik preset'ler her zaman vardır; aynı `id` ile ezilebilir, `hide = [...]` ile gizlenebilir.

```toml
[[preset]]
id = "web-jpg"
label = "JPG (web, 1920px)"
kind = "image"            # image | ffmpeg | pdf
output = "jpg"
input = ["image"]         # gruplar: image, audio, video, media, document; ya da tek tek uzantılar
suffix = "web"            # foto.png -> foto-web.jpg
quality = 80
max_width = 1920          # oran korunur, asla büyütülmez
background = "#ffffff"    # şeffaf alanın JPG'deki rengi

[[preset]]
id = "mp4-720p"
label = "MP4 (720p)"
kind = "ffmpeg"
output = "mp4"
input = ["video"]
suffix = "720p"
args = ["-vf", "scale=-2:720", "-c:v", "libx264", "-crf", "26", "-c:a", "aac"]
```

Bir preset, çıktısıyla aynı uzantıdaki dosyada yalnızca küçültme (`max_width`/`max_height`) veya `suffix` tanımlıysa görünür
(düz "MP4 → MP4" anlamsızdır, "MP4 → 720p MP4" anlamlıdır).

## Komut satırı

```
shock-convert list                       # menü girdileri
shock-convert presets                    # presets.toml yolu (yoksa örneği oluşturur)
shock-convert run jpg a.png b.webp       # dosyaları JPG'ye çevir
shock-convert run pdf rapor.docx         # PDF'e çevir
shock-convert register | unregister      # yalnızca sağ tık menüsünü yaz / sil
shock-convert install | uninstall        # kurulum / tam kaldırma
```

## Sorun giderme

Sağ tıkla başlatılan süreçte konsol olmadığı için hatalar `%LOCALAPPDATA%\ShockConvert\log.txt` dosyasına yazılır.
Bir şey çalışmıyorsa `powershell -ExecutionPolicy Bypass -File .\diagnose.ps1` çalıştırın: kayıt defterini,
dönüştürmeyi ve çoklu seçim benzetimini dener, raporu panoya kopyalar.

## Dönüştürme kuralları

| Konu | Davranış |
|---|---|
| Şeffaflık → JPG | Şeffaf alan beyaza oturtulur |
| Kalite | JPG kalitesi 90. WebP şimdilik **kayıpsız** yazılır (ileride kayıplı seçenek eklenebilir) |
| EXIF yönü | Uygulanır (telefon fotoğrafları doğru döner) |
| ICO | 16–256 px çoklu boyut; kaynaktan büyük boyutlar eklenmez, oran korunur |
| Animasyonlu GIF/WebP | Yalnızca ilk kare |
| Ses/video | ffmpeg ile; en fazla 2 dosya paralel (ffmpeg zaten çok çekirdek kullanır). Başlarken "dönüştürülüyor…" bildirimi çıkar. İlerleme çubuğu yok |
| Hız | Görüntülerde kazanç süreç içi dönüştürme ve hızlı başlangıçtan gelir. Ses/video süresini ffmpeg belirler, bu araç onu hızlandırmaz |
| Bozuk dosya | Hata bildirimi gösterilir, yarım çıktı bırakılmaz |

## Eklentiler (upscale vb. için)

Çekirdek Rust'tadır; ağır ya da özel işler (ör. yapay zekâ ile büyütme) harici program olarak eklenebilir.
`<kurulum>\plugins\<ad>\plugin.toml` oluşturun, sonra `shock-convert register` çalıştırın:

```toml
name = "upscale"

[[entry]]
id = "2x"
label = "Upscale 2x"
extensions = ["png", "jpg"]      # menünün görüneceği dosya türleri
output_suffix = "2x"             # isteğe bağlı: foto.png -> foto-2x.png (yoksa foto.png -> foto (1).png)
output_ext = "png"
command = ["python", "{plugin_dir}/upscale.py", "{input}", "{output}"]
```

`{input}`, `{output}` ve `{plugin_dir}` yer tutucuları doldurulur. Komut herhangi bir program olabilir
(Python betiği, `.exe`, vb.); sıfır dışı çıkış kodu hata sayılır ve son stderr satırı bildirimde gösterilir.
Eklenti kaldırılırken klasörü silip `register` çalıştırmak yeterlidir. Örnek: [`examples/plugins/grayscale`](examples/plugins/grayscale).

## Geliştirme

```
cargo test --workspace                                  # çekirdek + menü + eklenti testleri (Linux/Windows)
cargo check --workspace --target x86_64-pc-windows-gnu  # Windows kodunu Linux'tan derleme kontrolü
.\build.ps1                                             # Windows: exe + kurulum sihirbazı
```

```
crates/core      görüntü dönüştürme ve görüntü→PDF yazıcı (format, seçenekler, çıktı adlandırma)
crates/cli       shock-convert.exe: komutlar, preset'ler, ffmpeg, menü modeli, eklentiler, kayıt defteri, bildirim
crates/cli/src/pdf  belge→PDF: DOCX/XLSX/PPTX→HTML çözümleyiciler, tarayıcı ve LibreOffice çalıştırıcıları
site             tanıtım sitesi (React + Vite; bkz. docs/SITE.md)
assets           geçici uygulama ikonu (make_icon.py ile üretilir; icon.ico exe'ye gömülür)
installer        Inno Setup betiği
```

Kayıt defteri anahtarları (hepsi `HKCU`): `Software\Classes\SystemFileAssociations\.<uzantı>\shell\ShockConvert`
ve `Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert`. `unregister` yalnızca bunları siler, boş kalan üst anahtarları da temizler.

Windows'ta elle doğrulama listesi: [docs/WINDOWS-TEST.md](docs/WINDOWS-TEST.md).

## Lisans ve kod imzalama

[LICENSE](LICENSE) dosyasına bakın (Apache-2.0).

Kod imzalama politikası: [CODE_SIGNING.md](CODE_SIGNING.md) · Code signing policy: [CODE_SIGNING.md](CODE_SIGNING.md).
Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/) (başvuru aşamasında).
