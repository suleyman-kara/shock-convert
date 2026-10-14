export type Lang = "tr" | "en";

export const REPO = "https://github.com/suleyman-kara/shock-convert";

const tr = {
  nav: { features: "Özellikler", how: "Nasıl çalışır", download: "İndir", faq: "SSS" },
  hero: {
    badge: "Ücretsiz · Açık kaynak · Windows 10/11",
    title1: "Sağ tıkla,",
    title2: "dönüştür.",
    lead: "Görüntü, ses, video ve belgeleri Dosya Gezgini'nden çıkmadan istediğiniz biçime çevirin. Office gerekmez, dosyalarınız bilgisayarınızdan çıkmaz.",
    cta: "Windows için indir",
    secondary: "GitHub'da incele",
  },
  menu: { file: "foto.png", open: "Aç", copy: "Kopyala", rename: "Yeniden adlandır", props: "Özellikler", done: "foto.jpg hazır" },
  features: {
    title: "Tek menü, her biçim",
    items: [
      { icon: "🖼️", t: "Görüntü", d: "PNG, JPG, WebP, BMP, GIF, TIFF, ICO. Şeffaflık, EXIF yönü ve çok boyutlu ICO dahil." },
      { icon: "🎧", t: "Ses ve video", d: "MP3, WAV, FLAC, OGG, M4A, MP4, WebM ve videodan GIF. ffmpeg ile." },
      { icon: "📄", t: "PDF, Office olmadan", d: "Görüntü, metin ve Office belgelerini PDF'e çevirin; PDF'leri birleştirin, bölün, döndürün, küçültün, görüntüye ya da metne çevirin." },
      { icon: "⚡", t: "Hızlı ve hafif", d: "Rust ile yazılmış, birkaç megabaytlık tek program. Çoklu seçim paralel işlenir, tek bildirim gelir." },
      { icon: "🛡️", t: "Asla üzerine yazmaz", d: "foto.png → foto.jpg. Ad doluysa foto (1).jpg olur; mevcut dosyanız hiç değişmez." },
      { icon: "🧩", t: "Preset ve eklentiler", d: "\"Web için 1920px JPG\", \"720p MP4\" gibi kendi seçeneklerinizi ekleyin; eklentilerle genişletin." },
    ],
  },
  how: {
    title: "Üç adımda",
    steps: [
      { t: "Kurun", d: "Kurulum sihirbazı yönetici hakkı istemez; her şey kendi kullanıcı profilinize kurulur." },
      { t: "Sağ tıklayın", d: "Bir ya da birden çok dosya seçin, sağ tıklayın: Shock Convert ▸ istediğiniz biçim." },
      { t: "Hazır", d: "Dönüştürülen dosya aynı klasörde, aynı adla belirir. Bitince bildirim gelir." },
    ],
    note: "Windows 11'de menü şimdilik \"Daha fazla seçenek göster\" altında görünür.",
  },
  download: {
    title: "İndir",
    version: "Sürüm",
    setup: "Kurulum sihirbazı (.exe)",
    setupHint: "Önerilen. Yönetici hakkı gerekmez, Uygulamalar listesinden temizce kaldırılır.",
    portable: "Tek başına exe",
    portableHint: "Kurulumsuz; komut satırından shock-convert install ile de kurulabilir.",
    all: "Tüm sürümler ve sürüm notları",
    checksum: "SHA-256 özeti",
    reqTitle: "Gereksinimler",
    req: [
      "Windows 10 veya 11 (64 bit)",
      "Ses/video için ffmpeg — isteğe bağlı: winget install Gyan.FFmpeg",
      "DOC, XLS, PPT, ODT gibi eski belgeler için Microsoft Office ya da LibreOffice — isteğe bağlı",
    ],
    smartTitle: "\"Windows bilgisayarınızı korudu\" uyarısı",
    smart: "Kod imzalama başvurumuz sürerken Windows SmartScreen bu uyarıyı gösterebilir. \"Ek bilgi\" ▸ \"Yine de çalıştır\" ile devam edebilirsiniz. Dosyanın özetini yukarıdaki SHA-256 değeriyle karşılaştırabilirsiniz.",
  },
  trust: {
    title: "Gizlilik ve açık kaynak",
    items: [
      { t: "Dosyalarınız sizde kalır", d: "Her şey bilgisayarınızda çalışır. Hiçbir dosya ya da bilgi internete gönderilmez; telemetri yoktur." },
      { t: "Apache-2.0 lisanslı", d: "Kaynak kodun tamamı GitHub'da. İnceleyin, katkıda bulunun, hata bildirin." },
      { t: "Şeffaf derleme", d: "Kurulum dosyaları yalnızca GitHub Actions ile, herkese açık koddan üretilir." },
    ],
  },
  faq: {
    title: "Sık sorulan sorular",
    items: [
      { q: "Ücretli mi?", a: "Hayır. Shock Convert ücretsiz ve açık kaynaktır." },
      { q: "Kaldırınca geride bir şey kalır mı?", a: "Hayır. Kaldırma; menüyü, kayıt defteri girdilerini, program klasörünü ve günlük dosyasını siler. Yalnızca kendi düzenlediğiniz presets.toml dosyası korunur." },
      { q: "Belgeleri PDF'e çevirmek için Office gerekiyor mu?", a: "Hayır. Office yoksa Word, Excel ve PowerPoint dosyaları yerleşik dönüştürücüyle PDF'e çevrilir. Microsoft Office ya da LibreOffice kuruluysa daha yüksek doğruluk için otomatik olarak o kullanılır." },
      { q: "Ses ve video dönüştürme çalışmıyor.", a: "Ses ve video için ffmpeg gerekir. winget install Gyan.FFmpeg komutuyla kurup yeniden deneyin." },
      { q: "Bir sorun buldum, nereye bildireyim?", a: "GitHub'daki Issues sayfasına. %LOCALAPPDATA%\\ShockConvert\\log.txt dosyasını eklerseniz çok yardımcı olur." },
    ],
  },
  footer: { license: "Apache-2.0 lisansı", signing: "Kod imzalama politikası", issues: "Sorun bildir", releases: "Sürümler" },
};

type Dict = typeof tr;

const en: Dict = {
  nav: { features: "Features", how: "How it works", download: "Download", faq: "FAQ" },
  hero: {
    badge: "Free · Open source · Windows 10/11",
    title1: "Right-click,",
    title2: "convert.",
    lead: "Convert images, audio, video and documents without leaving File Explorer. No Office required, and your files never leave your computer.",
    cta: "Download for Windows",
    secondary: "View on GitHub",
  },
  menu: { file: "photo.png", open: "Open", copy: "Copy", rename: "Rename", props: "Properties", done: "photo.jpg is ready" },
  features: {
    title: "One menu, every format",
    items: [
      { icon: "🖼️", t: "Images", d: "PNG, JPG, WebP, BMP, GIF, TIFF, ICO — with transparency, EXIF orientation and multi-size ICO." },
      { icon: "🎧", t: "Audio & video", d: "MP3, WAV, FLAC, OGG, M4A, MP4, WebM and GIF from video, powered by ffmpeg." },
      { icon: "📄", t: "PDF without Office", d: "Turn images, text and Office documents into PDF; merge, split, rotate and compress PDFs, or turn them into images or text." },
      { icon: "⚡", t: "Fast and light", d: "A single few-megabyte program written in Rust. Multiple files run in parallel with one notification." },
      { icon: "🛡️", t: "Never overwrites", d: "photo.png → photo.jpg. If the name is taken you get photo (1).jpg; your files stay untouched." },
      { icon: "🧩", t: "Presets & plugins", d: "Add your own options like \"1920px JPG for web\" or \"720p MP4\", and extend it with plugins." },
    ],
  },
  how: {
    title: "Three steps",
    steps: [
      { t: "Install", d: "The installer needs no admin rights; everything goes into your own user profile." },
      { t: "Right-click", d: "Select one or more files, right-click: Shock Convert ▸ the format you want." },
      { t: "Done", d: "The converted file appears in the same folder with the same name, followed by a notification." },
    ],
    note: "On Windows 11 the menu currently appears under \"Show more options\".",
  },
  download: {
    title: "Download",
    version: "Version",
    setup: "Installer (.exe)",
    setupHint: "Recommended. No admin rights needed; uninstalls cleanly from Apps.",
    portable: "Standalone exe",
    portableHint: "No installation; can also be installed with shock-convert install.",
    all: "All releases and release notes",
    checksum: "SHA-256 checksum",
    reqTitle: "Requirements",
    req: [
      "Windows 10 or 11 (64-bit)",
      "ffmpeg for audio/video — optional: winget install Gyan.FFmpeg",
      "Microsoft Office or LibreOffice for legacy documents like DOC, XLS, PPT, ODT — optional",
    ],
    smartTitle: "\"Windows protected your PC\" warning",
    smart: "While our code signing application is in progress, Windows SmartScreen may show this warning. Choose \"More info\" ▸ \"Run anyway\" to continue. You can compare the file against the SHA-256 checksum above.",
  },
  trust: {
    title: "Privacy and open source",
    items: [
      { t: "Your files stay with you", d: "Everything runs on your computer. No files or data are sent anywhere, and there is no telemetry." },
      { t: "Apache-2.0 licensed", d: "All the source code is on GitHub. Read it, contribute, report issues." },
      { t: "Transparent builds", d: "Installers are built only by GitHub Actions from the public source code." },
    ],
  },
  faq: {
    title: "Frequently asked questions",
    items: [
      { q: "Is it paid?", a: "No. Shock Convert is free and open source." },
      { q: "Does uninstalling leave anything behind?", a: "No. Uninstalling removes the menu, registry entries, program folder and log file. Only your own edited presets.toml is kept." },
      { q: "Do I need Office to convert documents to PDF?", a: "No. Without Office, Word, Excel and PowerPoint files are converted by the built-in converter. If Microsoft Office or LibreOffice is installed it is used automatically for higher fidelity." },
      { q: "Audio and video conversion doesn't work.", a: "Audio and video need ffmpeg. Install it with winget install Gyan.FFmpeg and try again." },
      { q: "I found a problem — where do I report it?", a: "On the Issues page on GitHub. Attaching %LOCALAPPDATA%\\ShockConvert\\log.txt helps a lot." },
    ],
  },
  footer: { license: "Apache-2.0 license", signing: "Code signing policy", issues: "Report an issue", releases: "Releases" },
};

export const DICT: Record<Lang, Dict> = { tr, en };
