#!/usr/bin/env bash
# PDFium'u third_party/pdfium/<platform>/ altına indirir ve SHA-256 ile doğrular.
# Kullanım: scripts/fetch-pdfium.sh [linux-x64|win-x64]   (varsayılan linux-x64)
# Testler için: export SHOCK_CONVERT_PDFIUM="$PWD/third_party/pdfium/linux-x64/lib"
set -euo pipefail
cd "$(dirname "$0")/.."
platform="${1:-linux-x64}"
pin=third_party/pdfium.txt
version=$(sed -n 's/^version=//p' "$pin")
asset="pdfium-${platform}.tgz"
sha=$(sed -n "s/^${asset}=//p" "$pin")
[[ -n "$sha" ]] || { echo "$pin içinde $asset yok" >&2; exit 1; }
dest="third_party/pdfium/${platform}"
if [[ -f "$dest/.sha256" && "$(cat "$dest/.sha256")" == "$sha" ]]; then
  echo "PDFium zaten hazır: $dest"; exit 0
fi
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
curl -fsSL -o "$tmp" "https://github.com/bblanchon/pdfium-binaries/releases/download/${version/\//%2F}/${asset}"
echo "${sha}  ${tmp}" | sha256sum -c --quiet -
rm -rf "${dest:?}"
mkdir -p "$dest"
tar -xzf "$tmp" -C "$dest"
echo "$sha" > "$dest/.sha256"
echo "PDFium hazır: $dest"
