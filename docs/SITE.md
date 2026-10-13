# Tanıtım sitesi

Kaynak `site/` (React + Vite + TypeScript). `main`'e her birleşmede `site` iş akışı derleyip
GitHub Pages'e (`gh-pages` dalının kökü) yayınlar. Kurulum dosyaları aynı dalda `download/vX.Y.Z/`
altındadır (`publish-download` iş akışı); site indirme düğmesini çalışma anında
`download/latest.json` dosyasından okur, yani yeni sürüm için sitenin yeniden derlenmesi gerekmez.

```
cd site
npm ci
npm run dev      # http://localhost:5173
npm run build    # site/dist
```

Metinler `site/src/i18n.ts` içinde (Türkçe ve İngilizce). Dil, tarayıcı diline göre seçilir; üst
menüdeki TR/EN düğmesiyle değiştirilir.

## Alt alan adı bağlama (ör. `shockconvert.suleymankara.com`)

1. **DNS:** alan adınızın DNS panelinde bir `CNAME` kaydı ekleyin:
   `shockconvert` → `suleyman-kara.github.io` (sonunda repo adı **olmadan**).
2. **GitHub:** repo **Settings → Pages → Custom domain** alanına `shockconvert.suleymankara.com`
   yazıp kaydedin. GitHub `gh-pages` dalına bir `CNAME` dosyası ekler; iş akışlarımız bu dosyayı korur.
3. DNS doğrulanınca **Enforce HTTPS** kutusunu işaretleyin (sertifika birkaç dakika ile birkaç saat
   içinde hazır olur).
4. Önerilir: GitHub hesabınızın **Settings → Pages → Verified domains** bölümünden
   `suleymankara.com` alan adını doğrulayın. Böylece başka biri bu alan adına ait bir alt alan adını
   kendi deposuna bağlayamaz.

> **Microsoft Store uyarısı:** Özel alan adı bağlandığı anda
> `suleyman-kara.github.io/shock-convert/...` adresleri yeni alan adına **yönlendirilir**. Store'un
> paket bağlantısı yönlendirme kabul etmediği için Partner Center'daki Package URL'yi
> `https://shockconvert.suleymankara.com/download/vX.Y.Z/shock-convert-setup-X.Y.Z.exe` olarak
> güncelleyin.
