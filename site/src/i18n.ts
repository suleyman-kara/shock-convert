export const REPO = "https://github.com/suleyman-kara/shock-convert";

export const T = {
  nav: { features: "Features", how: "How it works", download: "Download", faq: "FAQ" },
  hero: {
    badge: "Free · Open source · Windows 10/11",
    title1: "Right-click,",
    title2: "convert.",
    lead: "Convert images to the format you need without leaving File Explorer. Your files never leave your computer.",
    cta: "Download for Windows",
    secondary: "View on GitHub",
  },
  menu: { file: "photo.png", open: "Open", copy: "Copy", rename: "Rename", props: "Properties", done: "photo.jpg is ready" },
  features: {
    title: "One menu, every image format",
    items: [
      { icon: "🖼️", t: "Seven formats", d: "PNG, JPG, WebP, BMP, GIF, TIFF and ICO — with transparency, EXIF orientation and multi-size ICO." },
      { icon: "🎛️", t: "Custom convert", d: "Pick format, quality and maximum size for a one-off conversion, then save it as a menu entry if you like." },
      { icon: "📍", t: "Your photo data, your call", d: "Keep EXIF, remove only the GPS location before sharing, or strip it all. Colour profiles are always kept." },
      { icon: "⚡", t: "Fast and light", d: "A single few-megabyte program written in Rust. Multiple files run in parallel with one notification." },
      { icon: "🛡️", t: "Never overwrites", d: "photo.png → photo.jpg. If the name is taken you get photo (1).jpg; your files stay untouched." },
      { icon: "🧩", t: "Presets & plugins", d: "Add your own options like \"1920px JPG for web\", and extend it with plugins." },
    ],
  },
  how: {
    title: "Three steps",
    steps: [
      { t: "Install", d: "The installer needs no admin rights; everything goes into your own user profile." },
      { t: "Right-click", d: "Select one or more images, right-click: Shock Convert ▸ the format you want." },
      { t: "Done", d: "The converted file appears in the same folder with the same name, followed by a notification." },
    ],
    note: "On Windows 11 the menu appears under \"Show more options\".",
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
    req: ["Windows 10 or 11 (64-bit)", "Nothing else to install"],
    smartTitle: "\"Windows protected your PC\" warning",
    smart: "Shock Convert is not code-signed, so Windows SmartScreen may show this warning. Choose \"More info\" ▸ \"Run anyway\" to continue. You can compare the file against the SHA-256 checksum above.",
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
      { q: "Can it convert audio, video or PDF?", a: "No. Shock Convert focuses on images only. Earlier versions had audio, video and PDF support; it was removed to keep the tool small and simple." },
      { q: "I found a problem — where do I report it?", a: "On the Issues page on GitHub. Attaching %LOCALAPPDATA%\\ShockConvert\\log.txt helps a lot." },
    ],
  },
  footer: { license: "Apache-2.0 license", issues: "Report an issue", releases: "Releases" },
};
