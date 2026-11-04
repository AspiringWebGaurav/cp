Add-Type -AssemblyName System.Drawing

$root = (Get-Location).Path
$sourcePath = Join-Path $root "public\cp-icon.jpg"
$img = [System.Drawing.Bitmap]::FromFile($sourcePath)

$pubDir = Join-Path $root "public"
$iconsDir = Join-Path $root "src-tauri\icons"

$img.Save((Join-Path $pubDir "icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$img.Save((Join-Path $iconsDir "icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)

$sizes = @(32, 64, 128)
foreach ($s in $sizes) {
    $res = New-Object System.Drawing.Bitmap $s, $s
    $g = [System.Drawing.Graphics]::FromImage($res)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($img, 0, 0, $s, $s)
    $g.Dispose()
    
    $outPng = Join-Path $iconsDir "$($s)x$($s).png"
    $res.Save($outPng, [System.Drawing.Imaging.ImageFormat]::Png)
    
    if ($s -eq 32) {
        $hIcon = $res.GetHicon()
        $icon = [System.Drawing.Icon]::FromHandle($hIcon)
        $icoPath = Join-Path $iconsDir "icon.ico"
        $fs = [System.IO.File]::Create($icoPath)
        $icon.Save($fs)
        $fs.Close()
    }
    $res.Dispose()
}

$img.Dispose()
Write-Host "Icons generated successfully!"
