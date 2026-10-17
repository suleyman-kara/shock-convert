#!/usr/bin/env bash
# winget için ilk sürümün manifestini üretir (sonraki sürümleri build.yml'deki "winget" işi gönderir).
#
#   packaging/winget/new-manifest.sh 0.2.0 [çıktı-klasörü]
#
# Kurulum dosyasını GitHub release'inden indirip SHA-256'sını hesaplar ve microsoft/winget-pkgs
# deposunun beklediği yapıda üç dosya yazar:
#   <çıktı>/manifests/s/SuleymanKara/ShockConvert/<sürüm>/SuleymanKara.ShockConvert*.yaml
set -euo pipefail

version="${1:?kullanım: $0 <sürüm> [çıktı-klasörü]}"
out="${2:-dist/winget}"
id="SuleymanKara.ShockConvert"
repo="suleyman-kara/shock-convert"
url="https://github.com/${repo}/releases/download/v${version}/shock-convert-setup-${version}.exe"
# installer/shock-convert.iss içindeki AppId; Inno kaldırma anahtarına "_is1" ekler.
product_code="{6F1B7C52-3D0A-4E8B-9A57-5C0D2E94B1A3}_is1"
schema="1.9.0"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fsSL "$url" -o "$tmp"
sha="$(sha256sum "$tmp" | cut -d' ' -f1 | tr 'a-f' 'A-F')"
date="$(gh release view "v${version}" --repo "$repo" --json publishedAt --jq '.publishedAt[0:10]' 2> /dev/null || date -u +%F)"

dir="${out}/manifests/s/SuleymanKara/ShockConvert/${version}"
mkdir -p "$dir"

cat > "${dir}/${id}.yaml" << EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.${schema}.schema.json
PackageIdentifier: ${id}
PackageVersion: ${version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: ${schema}
EOF

cat > "${dir}/${id}.installer.yaml" << EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.${schema}.schema.json
PackageIdentifier: ${id}
PackageVersion: ${version}
InstallerType: inno
Scope: user
InstallModes:
- silent
- silentWithProgress
UpgradeBehavior: install
ProductCode: '${product_code}'
ReleaseDate: ${date}
Installers:
- Architecture: x64
  InstallerUrl: ${url}
  InstallerSha256: ${sha}
ManifestType: installer
ManifestVersion: ${schema}
EOF

cat > "${dir}/${id}.locale.en-US.yaml" << EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.${schema}.schema.json
PackageIdentifier: ${id}
PackageVersion: ${version}
PackageLocale: en-US
Publisher: Suleyman Kara
PublisherUrl: https://github.com/suleyman-kara
PublisherSupportUrl: https://github.com/${repo}/issues
PackageName: Shock Convert
PackageUrl: https://shockconvert.suleymankara.com/
License: Apache-2.0
LicenseUrl: https://github.com/${repo}/blob/main/LICENSE
ShortDescription: Convert images from the File Explorer right-click menu.
Description: |-
  Shock Convert adds a right-click menu to File Explorer that converts images between PNG, JPG,
  WebP, BMP, GIF, TIFF and ICO. Output is written next to the source and never overwrites existing
  files. Supports custom presets, EXIF location removal and plugins. No admin rights needed.
Tags:
- image
- converter
- context-menu
- png
- jpg
- webp
ReleaseNotesUrl: https://github.com/${repo}/releases/tag/v${version}
ManifestType: defaultLocale
ManifestVersion: ${schema}
EOF

echo "Yazıldı: ${dir}"
ls -1 "$dir"
