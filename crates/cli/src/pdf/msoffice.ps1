# Shock Convert: Office belgesini PDF'e aktarir (msoffice.rs tarafindan calistirilir).
$ErrorActionPreference = 'Stop'
$in = $env:SC_IN; $out = $env:SC_OUT; $kind = $env:SC_APP; $proc = $env:SC_PROC
$before = @(Get-Process -Name $proc -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$app = $null; $doc = $null; $code = 0
try {
    $app = New-Object -ComObject $env:SC_PROGID
    $started = @(Get-Process -Name $proc -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object { $_.Id })
    Set-Content -LiteralPath $env:SC_PIDS -Value ($started -join ',') -Encoding ASCII
    switch ($kind) {
        'Word' {
            $app.Visible = $false
            $app.DisplayAlerts = 0
            $app.AutomationSecurity = 3
            # FileName, ConfirmConversions, ReadOnly, AddToRecentFiles, PasswordDocument
            $doc = $app.Documents.Open($in, $false, $true, $false, 'shock-convert-no-password')
            $doc.ExportAsFixedFormat($out, 17)
            $doc.Saved = $true
            $doc.Close()
        }
        'Excel' {
            $app.Visible = $false
            $app.DisplayAlerts = $false
            $app.ScreenUpdating = $false
            $app.AskToUpdateLinks = $false
            $app.AutomationSecurity = 3
            # FileName, UpdateLinks, ReadOnly, Format, Password
            $doc = $app.Workbooks.Open($in, 0, $true, [Type]::Missing, 'shock-convert-no-password')
            $doc.ExportAsFixedFormat(0, $out)
            $doc.Saved = $true
            $doc.Close($false)
        }
        'PowerPoint' {
            $app.DisplayAlerts = 1
            $app.AutomationSecurity = 3
            # FileName, ReadOnly, Untitled, WithWindow
            $doc = $app.Presentations.Open($in, -1, 0, 0)
            $doc.SaveAs($out, 32)
            $doc.Saved = -1
            $doc.Close()
        }
    }
    $doc = $null
} catch {
    Set-Content -LiteralPath $env:SC_ERR -Value $_.Exception.Message -Encoding UTF8
    $code = 1
} finally {
    if ($doc -ne $null) { try { $doc.Saved = $true; $doc.Close() } catch {} }
    if ($app -ne $null) {
        $ours = @($started).Count -gt 0
        $open = 0
        try {
            switch ($kind) {
                'Word' { $open = $app.Documents.Count }
                'Excel' { $open = $app.Workbooks.Count }
                'PowerPoint' { $open = $app.Presentations.Count }
            }
        } catch {}
        if ($ours -and $open -eq 0) { try { $app.Quit() } catch {} }
        [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
    }
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
}
exit $code
