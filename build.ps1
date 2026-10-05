# Windows'ta yerel derleme: exe + kurulum sihirbazı (Inno Setup 6 kuruluysa).
# Gereksinimler: Rust (rustup) + Visual Studio C++ Build Tools; sihirbaz için Inno Setup 6.
$ErrorActionPreference = "Stop"

# Yeni kurulan araçlar açık terminalin PATH'inde olmayabilir; cargo'nun varsayılan yolunu ekle.
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust (cargo) bulunamadi. Kurmak icin:" -ForegroundColor Red
    Write-Host "  winget install Rustlang.Rustup"
    Write-Host "  winget install Microsoft.VisualStudio.2022.BuildTools --override `"--passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`""
    Write-Host "Sonra terminali KAPATIP yeniden acin ve .\build.ps1 komutunu tekrar calistirin."
    exit 1
}

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$version = (Select-String -Path crates/cli/Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$candidates = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
)
$iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($iscc) {
    & $iscc "/DAppVersion=$version" installer\shock-convert.iss
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "Kurulum dosyasi: $(Resolve-Path dist\shock-convert-setup-$version.exe)" -ForegroundColor Green
} else {
    Write-Warning "Inno Setup 6 bulunamadi (winget install JRSoftware.InnoSetup). Sihirbaz uretilmedi."
    Write-Host "Yine de su exe ile kurabilirsiniz: .\target\release\shock-convert.exe install" -ForegroundColor Yellow
}
