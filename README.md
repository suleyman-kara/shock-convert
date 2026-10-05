# Shock Convert

Windows'ta görüntü, ses ve video dosyalarına **sağ tıklayıp** istediğiniz formata çeviren küçük ve hızlı bir araç.

```
foto.png  ──sağ tık──▶  Shock Convert ▸ JPG   ──▶  foto-jpg.jpg  (aynı klasörde)
```

- Görüntü: **PNG, JPG, WebP, BMP, GIF, TIFF, ICO** (girdi olarak `jpeg`, `jfif`, `tif` uzantıları da tanınır). Süreç içinde, harici program başlatmadan dönüştürülür.
- Ses/video: **MP3, WAV, FLAC, OGG, M4A, MP4, WebM, GIF (videodan)**. Bunlar için [ffmpeg](https://ffmpeg.org) gerekir (aşağıya bakın).
- Kendi **preset**'lerinizi `presets.toml` ile ekleyin: "web için 1920px JPG", "720p MP4" gibi.
- Birden fazla dosya seçilebilir; hepsi tek seferde, paralel işlenir ve tek bildirim gösterilir.
- Çıktı kaynağın yanına `ad-format.format` olarak yazılır. Ad doluysa `ad-format (1).format` olur, **mevcut dosyanın üzerine asla yazılmaz**.
- Yönetici hakkı gerekmez (her şey kullanıcı profiline kurulur). Windows 10 ve Windows 11'in klasik menüsünde ("Daha fazla seçenek göster") görünür.
- Tek bir küçük `.exe`; Python veya başka bir çalışma zamanı gerekmez.

## Kurulum / Kaldırma

**Kurulum sihirbazı ile:** `shock-convert-setup-<sürüm>.exe` dosyasını çalıştırın. Kaldırma için *Ayarlar ▸ Uygulamalar* listesinden "Shock Convert" seçilir; menü, kayıt defteri anahtarları ve dosyalar tamamen silinir.

**Sihirbazsız:** `shock-convert.exe install` kendini `%LOCALAPPDATA%\Programs\ShockConvert` altına kopyalar, menüyü kaydeder ve kaldırma girdisini ekler. `shock-convert.exe uninstall` hepsini geri alır.

> Menü, exe'nin bulunduğu yolu gösterir. Bu yüzden exe'yi kurulumdan sonra taşımayın; taşırsanız tekrar `install` çalıştırın.

## ffmpeg (ses/video için)

ffmpeg paketlenmez. Şu sırayla aranır: `presets.toml` içindeki `ffmpeg_path`, kurulum klasöründeki `ffmpeg\ffmpeg.exe`, `PATH`.
En kolay kurulum: `winget install Gyan.FFmpeg`. Bulunamazsa bildirimde bu komut gösterilir. Görüntü dönüştürme ffmpeg gerektirmez.

## Preset'ler

`shock-convert presets` dosya yolunu yazar ve yoksa açıklamalı bir örnek oluşturur (`%APPDATA%\ShockConvert\presets.toml`).
Düzenledikten sonra `shock-convert register` ile menüyü yenileyin. Yerleşik preset'ler her zaman vardır; aynı `id` ile ezilebilir, `hide = [...]` ile gizlenebilir.

```toml
[[preset]]
id = "web-jpg"
label = "JPG (web, 1920px)"
kind = "image"            # image | ffmpeg
output = "jpg"
input = ["image"]         # gruplar: image, audio, video, media; ya da tek tek uzantılar
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
output_suffix = "2x"             # foto.png -> foto-2x.png
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
crates/core   görüntü dönüştürme (format, seçenekler, çıktı adlandırma; Rust `image` crate'i)
crates/cli    shock-convert.exe: komutlar, preset'ler, ffmpeg, menü modeli, eklentiler, kayıt defteri, bildirim
installer     Inno Setup betiği
```

Kayıt defteri anahtarları (hepsi `HKCU`): `Software\Classes\SystemFileAssociations\.<uzantı>\shell\ShockConvert`
ve `Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert`. `unregister` yalnızca bunları siler, boş kalan üst anahtarları da temizler.

Windows'ta elle doğrulama listesi: [docs/WINDOWS-TEST.md](docs/WINDOWS-TEST.md).

## Lisans

[LICENSE](LICENSE) dosyasına bakın.
