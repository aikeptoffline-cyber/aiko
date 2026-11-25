# One-time setup: vendor the JS libraries (so the app runs offline) and make the app icon.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$cdn = "https://cdn.jsdelivr.net/npm"
$files = @{
  "three@0.169.0/build/three.module.js"                  = "ui/vendor/three/build/three.module.js"
  "three@0.169.0/examples/jsm/loaders/GLTFLoader.js"     = "ui/vendor/three/examples/jsm/loaders/GLTFLoader.js"
  "three@0.169.0/examples/jsm/utils/BufferGeometryUtils.js" = "ui/vendor/three/examples/jsm/utils/BufferGeometryUtils.js"
  "@pixiv/three-vrm@3.1.0/lib/three-vrm.module.min.js"   = "ui/vendor/three-vrm.module.min.js"
}
foreach ($src in $files.Keys) {
  $dst = $files[$src]
  New-Item -ItemType Directory -Force (Split-Path $dst) | Out-Null
  Invoke-WebRequest "$cdn/$src" -OutFile $dst -UseBasicParsing
  "fetched $dst"
}

# Tauri needs an .ico to build on Windows; draw a simple one.
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force src-tauri/icons | Out-Null
$bmp = New-Object System.Drawing.Bitmap 64, 64
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = "AntiAlias"
$g.FillEllipse((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(242, 155, 191))), 2, 2, 60, 60)
$g.Dispose()
$icon = [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
$fs = [System.IO.File]::Create("$PSScriptRoot/src-tauri/icons/icon.ico")
$icon.Save($fs); $fs.Close()
"made src-tauri/icons/icon.ico"
