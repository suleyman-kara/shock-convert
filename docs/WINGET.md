# winget yayını

Shock Convert, `SuleymanKara.ShockConvert` kimliğiyle winget üzerinden dağıtılır:

```
winget install SuleymanKara.ShockConvert
```

winget, GitHub release'indeki Inno kurulum dosyasını (`shock-convert-setup-X.Y.Z.exe`) indirip sessiz
kurar (`Scope: user`, yönetici hakkı gerekmez). Dosyayı tarayıcı indirmediği için SmartScreen uyarısı
çıkmaz; kod imzalama gerekmez. Microsoft her gönderimi antivirüs taramasından ve kurulum denemesinden
geçirir.

## Nasıl çalışır

- **İlk sürüm (bir kerelik, elle):** `packaging/winget/new-manifest.sh` manifesti üretir ve
  `microsoft/winget-pkgs` deposuna PR açılır. Microsoft'un doğrulaması ve bir moderatör onayı
  genelde birkaç gün sürer.
- **Sonraki sürümler (otomatik):** `main`'e birleşen her PR kararlı sürüm çıkarır; `build.yml`
  içindeki `winget` işi [winget-releaser](https://github.com/vedantmgoyal9/winget-releaser) ile
  yeni sürümün manifest PR'ını açar. Bu iş, paketin ilk sürümü winget'te olmadan çalışmaz.
  `WINGET_TOKEN` tanımlı değilse atlanır, sürümü bozmaz.

## Repo sahibinin yapacakları (bir kerelik)

1. **Fork:** https://github.com/microsoft/winget-pkgs → **Fork** (kendi hesabına, adı değiştirmeden).
2. **Token:** GitHub → Settings → Developer settings → Personal access tokens → **Tokens (classic)**
   → *Generate new token (classic)*. Kapsamlar: `public_repo` ve `workflow`. Süresiz yerine bir yıl
   seçip takvime yenileme hatırlatması koymak daha güvenli. (Fine-grained token desteklenmiyor.)
3. **Secret:** bu repoda Settings → Secrets and variables → Actions → **New repository secret**,
   ad `WINGET_TOKEN`, değer yukarıdaki token.
4. **İlk gönderim:** v0.2.0 yayınlandıktan sonra manifest üretilip `winget-pkgs`'e PR açılır
   (Claude hazırlar; gönderim senin onayınla, senin hesabından yapılır):

   ```
   packaging/winget/new-manifest.sh 0.2.0
   winget validate --manifest dist/winget/manifests/s/SuleymanKara/ShockConvert/0.2.0
   ```

   Üretilen klasör fork'taki aynı yola kopyalanır ve `microsoft/winget-pkgs`'e PR açılır.
5. PR'daki doğrulama yorumlarını izle; onaylanınca `winget install SuleymanKara.ShockConvert` çalışır.

## Notlar

- Manifestteki `ProductCode`, Inno'nun kaldırma anahtarıdır (`installer/shock-convert.iss` içindeki
  `AppId` + `_is1`). `AppId` değişirse winget yükseltme ve kaldırmayı eşleştiremez; değiştirilmemeli.
- winget yalnızca son 3 sürümü tutar (`max-versions-to-keep: 3`).
- Bir sürümü winget'e göndermemek için PR'a `release:skip` etiketi eklenir (sürüm hiç çıkmaz).
