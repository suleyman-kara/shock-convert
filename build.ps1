# Windows'ta yerel derleme: exe + kurulum sihirbazı (Inno Setup 6 kuruluysa).
$ErrorActionPreference = "Stop"
cargo build --release
$version = (Select-String -Path crates/cli/Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (Test-Path $iscc) {
    & $iscc "/DAppVersion=$version" installer\shock-convert.iss
    Write-Host "Kurulum dosyası: dist\shock-convert-setup-$version.exe"
} else {
    Write-Warning "Inno Setup 6 bulunamadı; yalnızca target\release\shock-convert.exe üretildi (shock-convert install ile kurulabilir)."
}
