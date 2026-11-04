Add-Type -AssemblyName System.Drawing
$srcPath = "C:\Users\gaura\.gemini\antigravity\brain\7aedb617-3326-44df-a9cb-285aee358cbe\.user_uploaded\media_1793768891902_1a744b49.png"
$src = [System.Drawing.Bitmap]::FromFile($srcPath)

# White CP Bounds: X=[15, 141], Y=[26, 118]
# Center of CP is X=78, Y=72
# Square size in source coordinates: 120 x 120
$cropSize = 126
$cropX = [Math]::Max(0, [int](78 - $cropSize / 2))
$cropY = [Math]::Max(0, [int](72 - $cropSize / 2))
if ($cropX + $cropSize -gt $src.Width) { $cropX = $src.Width - $cropSize }
if ($cropY + $cropSize -gt $src.Height) { $cropY = $src.Height - $cropSize }

$destSize = 512
$destBmp = New-Object System.Drawing.Bitmap($destSize, $destSize, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($destBmp)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality

# Rounded squircle path
$radius = 100
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$rect = New-Object System.Drawing.Rectangle(0, 0, $destSize, $destSize)
$d = $radius * 2
$path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
$path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
$path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
$path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
$path.CloseFigure()

$g.SetClip($path)
$srcRect = New-Object System.Drawing.Rectangle($cropX, $cropY, $cropSize, $cropSize)
$destRect = New-Object System.Drawing.Rectangle(0, 0, $destSize, $destSize)
$g.DrawImage($src, $destRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
$g.ResetClip()

# Subtle inner stroke for crisp definition
$pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(40, 255, 255, 255), 3)
$g.DrawPath($pen, $path)

$g.Dispose()
$src.Dispose()

$destBmp.Save("C:\GITHUB\cp\src\assets\cp-icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$destBmp.Save("C:\GITHUB\cp\public\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$destBmp.Dispose()
Write-Output "Successfully generated centered squircle icon 512x512"
