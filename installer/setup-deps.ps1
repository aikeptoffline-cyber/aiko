# Run by the installer after the app is copied: makes sure Ollama is running and downloads her models.
# Shows progress in its console window; on failure it says what to do instead of closing silently.
param([string]$Model = 'qwen3-vl:2b-instruct')
$ErrorActionPreference = 'Stop'
$Host.UI.RawUI.WindowTitle = 'AIKO setup: downloading her AI'

function Up {
  try { Invoke-RestMethod http://localhost:11434/api/tags -TimeoutSec 2 | Out-Null; $true } catch { $false }
}

try {
  Write-Host "`n  AIKO setup`n  ----------`n"
  $ollama = "$env:LOCALAPPDATA\Programs\Ollama\ollama.exe"
  if (-not (Test-Path $ollama)) {
    $found = Get-Command ollama -ErrorAction SilentlyContinue
    if (-not $found) { throw 'Ollama is not installed. Get it from https://ollama.com and run this setup again.' }
    $ollama = $found.Source
  }

  if (-not (Up)) {
    Write-Host '  Starting Ollama...'
    $app = Join-Path (Split-Path $ollama) 'ollama app.exe'
    if (Test-Path $app) { Start-Process $app } else { Start-Process $ollama -ArgumentList 'serve' -WindowStyle Hidden }
    for ($i = 0; $i -lt 30 -and -not (Up); $i++) { Start-Sleep -Seconds 1 }
    if (-not (Up)) { throw 'Ollama did not start. Start "Ollama" from the Start menu and run this setup again.' }
  }

  foreach ($m in $Model, 'nomic-embed-text') {
    Write-Host "`n  Downloading $m (only the first time; this can take a few minutes)`n"
    & $ollama pull $m
    if ($LASTEXITCODE -ne 0) { throw "Downloading $m failed. Check your internet connection and run this setup again." }
  }
  Write-Host "`n  All set. She's ready.`n" -ForegroundColor Green
  Start-Sleep -Seconds 3
} catch {
  Write-Host "`n  Setup could not finish: $($_.Exception.Message)`n" -ForegroundColor Red
  Read-Host '  Press Enter to close'
  exit 1
}
