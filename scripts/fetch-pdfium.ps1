# PDFium'u third_party\pdfium\win-x64\ altina indirir ve SHA-256 ile dogrular (build.ps1 ve CI kullanir).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$pin = Get-Content (Join-Path $root "third_party\pdfium.txt")
$version = ($pin | Where-Object { $_ -like "version=*" }) -replace "^version=", ""
$asset = "pdfium-win-x64.tgz"
$sha = ($pin | Where-Object { $_ -like "$asset=*" }) -replace "^$([regex]::Escape($asset))=", ""
$dest = Join-Path $root "third_party\pdfium\win-x64"
$stamp = Join-Path $dest ".sha256"
if ((Test-Path $stamp) -and ((Get-Content $stamp) -eq $sha)) { Write-Host "PDFium zaten hazir: $dest"; exit 0 }
$tmp = Join-Path ([IO.Path]::GetTempPath()) "shock-convert-$asset"
$url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$($version -replace '/', '%2F')/$asset"
Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
$actual = (Get-FileHash -Algorithm SHA256 $tmp).Hash.ToLowerInvariant()
if ($actual -ne $sha) { throw "PDFium SHA-256 uyusmuyor: $actual (beklenen $sha)" }
if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Force $dest | Out-Null
tar -xzf $tmp -C $dest
if ($LASTEXITCODE -ne 0) { throw "PDFium arsivi acilamadi" }
Set-Content -Path $stamp -Value $sha -Encoding ASCII
Remove-Item $tmp
Write-Host "PDFium hazir: $dest"
