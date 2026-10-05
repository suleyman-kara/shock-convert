# Shock Convert tani betigi: kurulumu, kayit defterini ve donusturmeyi dener,
# sonucu rapor olarak panoya kopyalar ve masaustune yazar. Calistirma:
#   powershell -ExecutionPolicy Bypass -File .\diagnose.ps1
$ErrorActionPreference = "Continue"
$out = New-Object System.Collections.Generic.List[string]
function Say($t) { $out.Add([string]$t); Write-Host $t }

$exe = Join-Path $env:LOCALAPPDATA "Programs\ShockConvert\shock-convert.exe"
$log = Join-Path $env:LOCALAPPDATA "ShockConvert\log.txt"
Say "=== Shock Convert tani raporu ($(Get-Date -Format s)) ==="
Say "Windows: $([Environment]::OSVersion.VersionString)"
Say "exe: $exe  var=$(Test-Path $exe)"
if (-not (Test-Path $exe)) { Say "HATA: kurulu exe bulunamadi."; }

# 1) Kayit defteri: .webp ve .png icin menu anahtarlari
foreach ($ext in @(".webp", ".png")) {
    Say ""
    Say "--- Kayit defteri: $ext ---"
    $base = "HKCU:\Software\Classes\SystemFileAssociations\$ext\shell\ShockConvert"
    if (Test-Path $base) {
        Get-ChildItem $base -Recurse | ForEach-Object {
            $k = $_
            Say $k.Name
            foreach ($n in $k.GetValueNames()) {
                $label = if ($n -eq "") { "(Default)" } else { $n }
                Say ("    {0} = {1}" -f $label, $k.GetValue($n))
            }
        }
    } else { Say "anahtar YOK: $base" }
}

# 2) Gercek test gorseli olustur (PNG + BMP), sandbox klasorunde
$work = Join-Path $env:TEMP "shock-convert-diag"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item $work -ItemType Directory | Out-Null
Add-Type -AssemblyName System.Drawing
function NewImg($path, $fmt) {
    $bmp = New-Object System.Drawing.Bitmap 120, 80
    $g = [System.Drawing.Graphics]::FromImage($bmp); $g.Clear([System.Drawing.Color]::SteelBlue); $g.Dispose()
    $bmp.Save($path, $fmt); $bmp.Dispose()
}
$png = Join-Path $work "Foto Bir.png"
NewImg $png ([System.Drawing.Imaging.ImageFormat]::Png)
foreach ($i in 1..3) { NewImg (Join-Path $work "coklu $i.png") ([System.Drawing.Imaging.ImageFormat]::Png) }

if (Test-Path $log) { Remove-Item $log -Force -ErrorAction SilentlyContinue }

# 3) Dogrudan komut: exe run jpg dosya
Say ""
Say "--- Test A: dogrudan 'run jpg' ---"
$p = Start-Process -FilePath $exe -ArgumentList @("run", "jpg", "`"$png`"") -Wait -PassThru
Say "cikis kodu: $($p.ExitCode)"
Say ("olusan: " + ((Get-ChildItem $work -Filter "Foto Bir-*" | ForEach-Object Name) -join ", "))

# 4) Kayit defterindeki komutun birebir calistirilmasi (Explorer'in yaptigi gibi, %1 yerine dosya)
Say ""
Say "--- Test B: kayit defterindeki komut (png -> jpg) ---"
$cmdKey = Get-ChildItem "HKCU:\Software\Classes\SystemFileAssociations\.png\shell\ShockConvert\shell" -ErrorAction SilentlyContinue |
    Where-Object { $_.PSChildName -like "*-jpg" } | Select-Object -First 1
if ($cmdKey) {
    $cmdline = (Get-Item (Join-Path $cmdKey.PSPath "command")).GetValue("")
    Say "komut: $cmdline"
    $b = Join-Path $work "Kayit Testi.png"
    Copy-Item $png $b
    $line = $cmdline.Replace("%1", $b).Replace("%L", $b)
    $p = Start-Process -FilePath "cmd.exe" -ArgumentList @("/c", "`"$line`"") -Wait -PassThru
    Start-Sleep -Milliseconds 800
    Say "cikis kodu: $($p.ExitCode)"
    Say ("olusan: " + ((Get-ChildItem $work -Filter "Kayit Testi-*" | ForEach-Object Name) -join ", "))

    # 5) Coklu secim benzetimi: 3 sureci ayni anda baslat
    Say ""
    Say "--- Test C: 3 sureci ayni anda (coklu secim benzetimi) ---"
    $procs = foreach ($i in 1..3) {
        $f = Join-Path $work "coklu $i.png"
        $l = $cmdline.Replace("%1", $f)
        Start-Process -FilePath "cmd.exe" -ArgumentList @("/c", "`"$l`"") -PassThru
    }
    $procs | Wait-Process -Timeout 30 -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Say ("olusan: " + ((Get-ChildItem $work -Filter "coklu*-jpg*" | ForEach-Object Name) -join ", "))
} else { Say "png icin jpg menu girdisi bulunamadi (menu kayitli degil)." }

# 6) Gunluk
Say ""
Say "--- Gunluk: $log ---"
if (Test-Path $log) { Get-Content $log -Tail 60 -Encoding UTF8 | ForEach-Object { Say $_ } } else { Say "gunluk dosyasi olusmadi (exe hic calismamis olabilir)" }

# 7) Surum / Windows Defender engeli ipucu
Say ""
Say "--- exe bilgisi ---"
if (Test-Path $exe) { Say ((Get-Item $exe) | Format-List Length, LastWriteTime | Out-String) }

$text = $out -join "`r`n"
$desktop = [Environment]::GetFolderPath("Desktop")
$text | Set-Content -Path (Join-Path $desktop "shock-convert-rapor.txt") -Encoding UTF8
try { Set-Clipboard -Value $text; Write-Host "`nRapor panoya kopyalandi ve masaustune shock-convert-rapor.txt olarak yazildi. Panodakini yapistirin." -ForegroundColor Green }
catch { Write-Host "`nPano kullanilamadi; rapor masaustundeki shock-convert-rapor.txt dosyasinda." -ForegroundColor Yellow }
