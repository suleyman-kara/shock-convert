#!/usr/bin/env bash
# Bu derlemenin sürümünü ve ne yayınlanacağını belirler (build.yml "version" işi).
#
# Girdi (ortam değişkenleri):
#   EVENT      push | pull_request | workflow_dispatch
#   REF        github.ref (refs/heads/main, refs/tags/v1.2.3 …)
#   PR_NUMBER  pull_request'te PR numarası
#   SAME_REPO  pull_request'te PR bu repodan mı (true/false); çatallardan gelen PR yayın yapamaz
#   LABELS     PR etiketleri, virgül ya da satırla ayrılmış (release:major, release:minor, release:skip)
#
# Çıktı (GITHUB_OUTPUT ya da stdout):
#   version  derlemeye yazılacak sürüm (ör. 0.2.1 ya da 0.2.1-pr.17)
#   tag      yayın etiketi (v0.2.1 ya da pr-17); yayın yoksa boş
#   kind     stable | tag | pr | none
#
# Kurallar:
#   - main'e her birleştirme en son vX.Y.Z etiketinden bir yama (patch) sürümü çıkarır.
#     PR'da release:minor / release:major etiketi varsa o basamak artar.
#   - crates/cli/Cargo.toml'daki sürüm daha yüksekse o kullanılır (elle büyük atlama için).
#   - Yalnızca belge/site/iş akışı değiştiyse ya da release:skip etiketi varsa sürüm çıkmaz.
#   - Elle itilen vX.Y.Z etiketi o sürümü yayınlar.
#   - Aynı repodan açılan PR, "pr-N" adlı bir ön sürüm (pre-release) üretir.
set -euo pipefail

out="${GITHUB_OUTPUT:-/dev/stdout}"
emit() { printf '%s=%s\n' "$1" "$2" >> "$out"; }

latest=$(git tag -l 'v*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sed 's/^v//' | sort -V | tail -1 || true)
cargo=$(sed -n 's/^version = "\([^"]*\)"/\1/p' crates/cli/Cargo.toml | head -1)

LABELS=$(tr ',' '\n' <<< "${LABELS:-}" | sed 's/^ *//; s/ *$//')
bump=patch
grep -qx 'release:minor' <<< "${LABELS:-}" && bump=minor
grep -qx 'release:major' <<< "${LABELS:-}" && bump=major

if [[ -z "$latest" ]]; then
  next="$cargo"
else
  IFS=. read -r ma mi pa <<< "$latest"
  case "$bump" in
    major) next="$((ma + 1)).0.0" ;;
    minor) next="${ma}.$((mi + 1)).0" ;;
    *) next="${ma}.${mi}.$((pa + 1))" ;;
  esac
  higher=$(printf '%s\n%s\n' "$next" "$cargo" | sort -V | tail -1)
  [[ "$higher" == "$cargo" ]] && next="$cargo"
fi

# Uygulamayı etkileyen bir değişiklik var mı (son sürümden bu yana)?
app_changed() {
  [[ -z "$latest" ]] && return 0
  git diff --name-only "v${latest}" HEAD |
    grep -Evq '^(site/|docs/|\.github/)|\.md$|^LICENSE$|^\.gitignore$'
}

case "${EVENT:-}:${REF:-}" in
  push:refs/tags/v*)
    v="${REF#refs/tags/v}"
    emit version "$v"; emit tag "v$v"; emit kind tag ;;
  push:refs/heads/main)
    if grep -qx 'release:skip' <<< "${LABELS:-}"; then
      echo "release:skip etiketi: sürüm çıkmıyor" >&2
      emit version "$next"; emit tag ""; emit kind none
    elif ! app_changed; then
      echo "Yalnızca belge/site/iş akışı değişti: sürüm çıkmıyor" >&2
      emit version "$next"; emit tag ""; emit kind none
    else
      emit version "$next"; emit tag "v$next"; emit kind stable
    fi ;;
  pull_request:*)
    emit version "${next}-pr.${PR_NUMBER}"
    if [[ "${SAME_REPO:-false}" == "true" ]]; then
      emit tag "pr-${PR_NUMBER}"; emit kind pr
    else
      emit tag ""; emit kind none
    fi ;;
  *)
    emit version "$next"; emit tag ""; emit kind none ;;
esac
