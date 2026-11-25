# Builds the app icon from her keyed master pose: her head on a round background, as
# src-tauri/icons/icon.ico (16-256 px, PNG-compressed entries) plus icon.png (512 px) for the installer.
param([string]$Sprite = "$PSScriptRoot\..\ui\sprites\master.png", [string]$Out = "$PSScriptRoot\..\src-tauri\icons")
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# her head and headset on the 687x1024 canvas
$crop = New-Object System.Drawing.Rectangle 148, 70, 392, 392

function Render([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'; $g.PixelOffsetMode = 'HighQuality'
  $circle = New-Object System.Drawing.Drawing2D.GraphicsPath
  $circle.AddEllipse(0, 0, $size - 1, $size - 1)
  $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush((New-Object System.Drawing.Point 0, 0), (New-Object System.Drawing.Point 0, $size),
    [System.Drawing.Color]::FromArgb(255, 72, 196, 189), [System.Drawing.Color]::FromArgb(255, 44, 52, 120))
  $g.FillPath($bg, $circle)
  $g.SetClip($circle)
  # the sprite's irises live in a separate layer (for eye tracking), so draw both
  foreach ($layer in (Resolve-Path $Sprite).Path, (Resolve-Path $Sprite).Path.Replace('.png', '-iris.png')) {
    $src = [System.Drawing.Image]::FromFile($layer)
    $g.DrawImage($src, (New-Object System.Drawing.Rectangle 0, ([int]($size * 0.04)), $size, $size), $crop, [System.Drawing.GraphicsUnit]::Pixel)
    $src.Dispose()
  }
  $g.ResetClip()
  $ring = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(220, 255, 255, 255)), ([Math]::Max(1, $size / 48))
  $g.DrawEllipse($ring, $ring.Width / 2, $ring.Width / 2, $size - 1 - $ring.Width, $size - 1 - $ring.Width)
  $g.Dispose()
  return $bmp
}

New-Item -ItemType Directory -Force $Out | Out-Null
$outDir = (Resolve-Path $Out).Path
$big = Render 512; $big.Save("$outDir\icon.png", [System.Drawing.Imaging.ImageFormat]::Png); $big.Dispose()

# .ico with PNG-compressed images (supported since Windows Vista)
$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = foreach ($s in $sizes) { $b = Render $s; $ms = New-Object IO.MemoryStream; $b.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); $b.Dispose(); , $ms.ToArray() }
$fs = [IO.File]::Create("$outDir\icon.ico"); $w = New-Object IO.BinaryWriter $fs
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
  $s = $sizes[$i]; $d = if ($s -ge 256) { 0 } else { $s }
  $w.Write([byte]$d); $w.Write([byte]$d); $w.Write([byte]0); $w.Write([byte]0)
  $w.Write([UInt16]1); $w.Write([UInt16]32); $w.Write([UInt32]$pngs[$i].Length); $w.Write([UInt32]$offset)
  $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $w.Write($p) }
$w.Close()
"icon.ico ($($sizes -join ', ') px) and icon.png written to $outDir"
