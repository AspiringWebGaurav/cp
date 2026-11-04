Add-Type -AssemblyName System.Drawing

$root = (Get-Location).Path
$img1Source = "C:\Users\gaura\.gemini\antigravity\brain\7aedb617-3326-44df-a9cb-285aee358cbe\.user_uploaded\media_1793768891902_1a744b49.png"
$img2Source = "C:\Users\gaura\.gemini\antigravity\brain\7aedb617-3326-44df-a9cb-285aee358cbe\.user_uploaded\media_1793768900107_7e0a5766.png"

# Ensure target directories exist
$pubDir = Join-Path $root "public"
$assetsDir = Join-Path $root "src\assets"
$iconsDir = Join-Path $root "src-tauri\icons"

if (-not (Test-Path $assetsDir)) { New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null }
if (-not (Test-Path $iconsDir)) { New-Item -ItemType Directory -Force -Path $iconsDir | Out-Null }
if (-not (Test-Path $pubDir)) { New-Item -ItemType Directory -Force -Path $pubDir | Out-Null }

# 1. Process 2nd image: Brand Logo for Website / Docs
Copy-Item -Path $img2Source -Destination (Join-Path $pubDir "brand-logo.png") -Force
Copy-Item -Path $img2Source -Destination (Join-Path $assetsDir "brand-logo.png") -Force
Write-Host "Brand logo for website saved to public/brand-logo.png and src/assets/brand-logo.png"

# 2. Process 1st image: App Logo & Windows Icons
Copy-Item -Path $img1Source -Destination (Join-Path $pubDir "icon.png") -Force
Copy-Item -Path $img1Source -Destination (Join-Path $assetsDir "cp-icon.png") -Force

# Generate resized PNGs and ICO from Image 1
$img = [System.Drawing.Bitmap]::FromFile($img1Source)
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
Write-Host "Application logo and Windows icons generated and injected successfully!"

