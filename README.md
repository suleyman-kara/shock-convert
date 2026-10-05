# Shock Convert

Windows'ta görüntü dosyalarına **sağ tıklayıp** istediğiniz formata çeviren küçük ve hızlı bir araç.

```
foto.png  ──sağ tık──▶  Shock Convert ▸ JPG   ──▶  foto-jpg.jpg  (aynı klasörde)
```

- Formatlar: **PNG, JPG, WebP, BMP, GIF, TIFF, ICO** (girdi olarak `jpeg`, `jfif`, `tif` uzantıları da tanınır)
- Birden fazla dosya seçilebilir; hepsi tek seferde, paralel işlenir ve tek bildirim gösterilir.
- Çıktı kaynağın yanına `ad-format.format` olarak yazılır. Ad doluysa `ad-format (1).format` olur, **mevcut dosyanın üzerine asla yazılmaz**.
- Yönetici hakkı gerekmez (her şey kullanıcı profiline kurulur). Windows 10 ve Windows 11'in klasik menüsünde ("Daha fazla seçenek göster") görünür.
- Tek bir küçük `.exe`; Python veya başka bir çalışma zamanı gerekmez.

## Kurulum / Kaldırma

**Kurulum sihirbazı ile:** `shock-convert-setup-<sürüm>.exe` dosyasını çalıştırın. Kaldırma için *Ayarlar ▸ Uygulamalar* listesinden "Shock Convert" seçilir; menü, kayıt defteri anahtarları ve dosyalar tamamen silinir.

**Sihirbazsız:** `shock-convert.exe install` kendini `%LOCALAPPDATA%\Programs\ShockConvert` altına kopyalar, menüyü kaydeder ve kaldırma girdisini ekler. `shock-convert.exe uninstall` hepsini geri alır.

> Menü, exe'nin bulunduğu yolu gösterir. Bu yüzden exe'yi kurulumdan sonra taşımayın; taşırsanız tekrar `install` çalıştırın.

## Komut satırı

```
shock-convert list                       # menü girdileri
shock-convert run jpg a.png b.webp       # dosyaları JPG'ye çevir
shock-convert register | unregister      # yalnızca sağ tık menüsünü yaz / sil
shock-convert install | uninstall        # kurulum / tam kaldırma
```

## Dönüştürme kuralları

| Konu | Davranış |
|---|---|
| Şeffaflık → JPG | Şeffaf alan beyaza oturtulur |
| Kalite | JPG kalitesi 90. WebP şimdilik **kayıpsız** yazılır (ileride kayıplı seçenek eklenebilir) |
| EXIF yönü | Uygulanır (telefon fotoğrafları doğru döner) |
| ICO | 16–256 px çoklu boyut; kaynaktan büyük boyutlar eklenmez, oran korunur |
| Animasyonlu GIF/WebP | Yalnızca ilk kare |
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
crates/core   dönüştürme mantığı (format, çıktı adlandırma, Pillow'suz saf Rust `image` crate'i)
crates/cli    shock-convert.exe: komutlar, menü modeli, eklentiler, kayıt defteri, bildirim
installer     Inno Setup betiği
```

Kayıt defteri anahtarları (hepsi `HKCU`): `Software\Classes\SystemFileAssociations\.<uzantı>\shell\ShockConvert`
ve `Software\Microsoft\Windows\CurrentVersion\Uninstall\ShockConvert`. `unregister` yalnızca bunları siler, boş kalan üst anahtarları da temizler.

Windows'ta elle doğrulama listesi: [docs/WINDOWS-TEST.md](docs/WINDOWS-TEST.md).

## Lisans

[LICENSE](LICENSE) dosyasına bakın.
