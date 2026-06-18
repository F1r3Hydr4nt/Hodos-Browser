<#
  open-hodos-wapps-demo.ps1
  ---------------------------------------------------------------------------
  One-shot launcher for the BOLT demo. Brings up the background stack (if it
  isn't already running) and opens all four demo wapps as TABS in a single
  Hodos window, using Hodos's own session-restore mechanism.

      catpicz  http://localhost:3001   (identity / gated cat download)
      bwanq    http://localhost:3002   (bank / IRP loan)
      tackle   http://localhost:3003   (shop / coupons)
      bucket   http://localhost:3004   (exchange / coupon sale)

  How the tabs work: Hodos reads <profile>/session.json on a fresh start
  (gated by browser.restoreSessionOnStart) and recreates each saved tab in one
  window, then deletes the file. So we synthesise session.json + enable the
  setting, then launch a fresh instance. The wapp pages call the integrated
  Hodos wallet directly at 127.0.0.1:31301/bolt/* (CORS-allowed).

  Usage:   pwsh -File .\open-hodos-wapps-demo.ps1            # hermetic (default)
           pwsh -File .\open-hodos-wapps-demo.ps1 -Regtest   # local SV node (real chain)
           # teratestnet (set the funded source + Arcade first):
           $env:ARCADE_URL='https://<arcade>'; $env:FUNDING_WIF='<wif>'; $env:FUNDING_OUTPOINT='<txid:vout:value>'
           pwsh -File .\open-hodos-wapps-demo.ps1 -Testnet
#>
param(
  [switch]$Regtest,   # demo against the LOCAL regtest SV node (real chain, no external deps)
  [switch]$Testnet    # demo against teratestnet via Arcade (needs ARCADE_URL + FUNDING_WIF + FUNDING_OUTPOINT)
)

$ErrorActionPreference = 'Continue'
$env:HODOS_DEV = '1'   # wallet + CEF dev-mode: data -> %APPDATA%\HodosBrowserDev

$HODOS  = $PSScriptRoot
$WAPPS  = Join-Path $HODOS '..\spv-demo-wapps'
# The project links to HodosBrowser.exe (HodosBrowserShell.exe is a stale legacy artifact).
$CEF    = Join-Path $HODOS 'cef-native\build\bin\Release\HodosBrowser.exe'
$CEFDIR = Split-Path $CEF

$sites = @(
  @{ name = 'catpicz'; url = 'http://localhost:3001' },
  @{ name = 'bwanq';   url = 'http://localhost:3002' },
  @{ name = 'tackle';  url = 'http://localhost:3003' },
  @{ name = 'bucket';  url = 'http://localhost:3004' }
)

function Test-Up([string]$url) {
  try { return (Invoke-WebRequest $url -TimeoutSec 3 -UseBasicParsing).StatusCode -ge 200 }
  catch { return $false }
}
function Wait-Up([string]$url, [int]$timeoutSec) {
  $deadline = (Get-Date).AddSeconds($timeoutSec)
  while ((Get-Date) -lt $deadline) { if (Test-Up $url) { return $true }; Start-Sleep -Seconds 2 }
  return (Test-Up $url)
}
function Stat([string]$url) { if (Test-Up $url) { 'UP' } else { 'DOWN' } }

Write-Host '== BOLT demo: bringing up the background stack ==' -ForegroundColor Cyan

# ---- chain backend: default hermetic | -Regtest (local SV node) | -Testnet (ttn Arcade) ----
$composeArgs = @('up', '-d')
$bringUpNode = $false
if ($Regtest) {
  $env:CHAIN_BACKEND = 'local'
  $env:RPC_HOST = 'host.docker.internal'; $env:RPC_PORT = '18332'
  $env:RPC_USERNAME = 'bitcoin'; $env:RPC_PASSWORD = 'lololol'
  $composeArgs = @('up', '-d', '--build'); $bringUpNode = $true
  Write-Host '   mode: REGTEST — local SV node, real broadcast/mine/BUMP/SPV' -ForegroundColor Yellow
} elseif ($Testnet) {
  if (-not $env:ARCADE_URL -or -not $env:FUNDING_WIF -or -not $env:FUNDING_OUTPOINT) {
    Write-Host '   !! -Testnet needs ARCADE_URL + FUNDING_WIF + FUNDING_OUTPOINT env set first' -ForegroundColor Red
    return
  }
  $env:CHAIN_BACKEND = 'ttn'
  $composeArgs = @('up', '-d', '--build')
  Write-Host '   mode: TESTNET (ttn) — Arcade broadcast + headers, pool-funded mints' -ForegroundColor Yellow
} else {
  $env:CHAIN_BACKEND = 'shared'
  Write-Host '   mode: hermetic (default)'
}

# Regtest: bring up the local SV node (repo-root compose) so sites/chain can reach it.
if ($bringUpNode) {
  Write-Host '   -> regtest node (docker compose up -d node @ repo root)...'
  Push-Location (Join-Path $HODOS '..'); docker compose up -d node | Out-Null; Pop-Location
}

# 1) Docker: demo sites (3001-3004) + shared chain service (3010). A net switch forces --build
# (so the chain image ships the latest /funding) and a restart even if the stack is already up.
if ($Regtest -or $Testnet -or -not (Test-Up 'http://localhost:3001')) {
  Write-Host ("   -> docker compose {0} (sites + chain + proxy)..." -f ($composeArgs -join ' '))
  Push-Location $WAPPS; docker compose @composeArgs | Out-Null; Pop-Location
  Wait-Up 'http://localhost:3001' 180 | Out-Null
}
Write-Host ("   sites    : {0}" -f (Stat 'http://localhost:3001'))
Write-Host ("   chain    : {0}" -f (Stat 'http://localhost:3010/health'))

# 2) Integrated Hodos wallet (:31301, HODOS_DEV dev data dir)
if (-not (Test-Up 'http://127.0.0.1:31301/bolt/identity')) {
  Write-Host '   -> starting integrated wallet (HODOS_DEV=1)...'
  $rw  = Join-Path $HODOS 'rust-wallet'
  $exe = Join-Path $rw 'target\release\hodos-wallet.exe'
  if (-not (Test-Path $exe)) {
    Write-Host '      building wallet (cargo build --release, ~4 min cold)...'
    Push-Location $rw; cargo build --release; Pop-Location
  }
  Start-Process -FilePath $exe -WorkingDirectory $rw -WindowStyle Minimized
  Wait-Up 'http://127.0.0.1:31301/bolt/identity' 60 | Out-Null
}
Write-Host ("   wallet   : {0}" -f (Stat 'http://127.0.0.1:31301/bolt/identity'))

# 3) Hodos frontend (the browser's own UI shell on :5137)
if (-not (Test-Up 'http://127.0.0.1:5137/')) {
  Write-Host '   -> starting Hodos frontend (vite :5137)...'
  Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','npx vite --port 5137' `
    -WorkingDirectory (Join-Path $HODOS 'frontend')
  Wait-Up 'http://127.0.0.1:5137/' 90 | Out-Null
}
Write-Host ("   frontend : {0}" -f (Stat 'http://127.0.0.1:5137/'))

# ---------------------------------------------------------------------------
# 4) Seed Hodos's session-restore so the 4 wapps come back as TABS in 1 window
# ---------------------------------------------------------------------------
if (-not (Test-Path $CEF)) {
  Write-Host "   !! CEF shell not built: $CEF" -ForegroundColor Yellow
  Write-Host '      build it: cd cef-native; .\win_build_run.ps1   (then re-run this script)' -ForegroundColor Yellow
  return
}

Write-Host '== seeding session-restore (4 tabs, one window) ==' -ForegroundColor Cyan

# Restore only runs on a FRESH first instance, so close any running Hodos first
# (both the current HodosBrowser.exe and any stale HodosBrowserShell.exe).
$running = Get-Process HodosBrowser,HodosBrowserShell -ErrorAction SilentlyContinue
if ($running) {
  Write-Host '   -> closing running Hodos so it restarts fresh...'
  $running | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
}

# Build session.json (v2: one window, four tabs) — matches SaveSession() output.
$tabsJson = ($sites | ForEach-Object {
  '    { "url": "' + $_.url + '", "title": "' + $_.name + '" }'
}) -join ",`n"
$sessionJson = @"
{
  "version": 2,
  "windows": [
    {
      "activeTabIndex": 0,
      "tabs": [
$tabsJson
      ]
    }
  ]
}
"@

# Minimal settings.json — WITH_DEFAULT means unspecified keys keep their defaults;
# we only need to flip restoreSessionOnStart on.
$settingsJson = @"
{
  "version": 1,
  "browser": { "restoreSessionOnStart": true }
}
"@

# Write to both possible data roots (HODOS_DEV -> HodosBrowserDev, else HodosBrowser),
# so it works regardless of how the shell resolves its data dir.
foreach ($root in 'HodosBrowserDev','HodosBrowser') {
  $profileDir = Join-Path $env:APPDATA (Join-Path $root 'Default')
  New-Item -ItemType Directory -Force -Path $profileDir | Out-Null
  Set-Content -Path (Join-Path $profileDir 'session.json')  -Value $sessionJson -Encoding UTF8
  # Only create settings.json if absent, so we don't clobber real user settings;
  # if it exists, enable the flag in place.
  $settingsPath = Join-Path $profileDir 'settings.json'
  if (-not (Test-Path $settingsPath)) {
    Set-Content -Path $settingsPath -Value $settingsJson -Encoding UTF8
  } else {
    try {
      $s = Get-Content $settingsPath -Raw | ConvertFrom-Json
      if (-not $s.browser) { $s | Add-Member -NotePropertyName browser -NotePropertyValue ([pscustomobject]@{}) -Force }
      $s.browser | Add-Member -NotePropertyName restoreSessionOnStart -NotePropertyValue $true -Force
      ($s | ConvertTo-Json -Depth 10) | Set-Content -Path $settingsPath -Encoding UTF8
    } catch {
      Set-Content -Path $settingsPath -Value $settingsJson -Encoding UTF8
    }
  }
  Write-Host ("   seeded: {0}" -f $profileDir)
}

# 5) Launch a fresh Hodos — it restores the 4 tabs in one window, then deletes session.json
Write-Host '== launching Hodos (restoring tabs) ==' -ForegroundColor Cyan
Start-Process -FilePath $CEF -WorkingDirectory $CEFDIR

# bring the window to the foreground once it appears
$deadline = (Get-Date).AddSeconds(45)
$win = $null
while ((Get-Date) -lt $deadline) {
  $win = Get-Process HodosBrowser -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
  if ($win) { break }
  Start-Sleep -Seconds 2
}
if ($win) {
  Add-Type -Name W -Namespace Fg -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);' -ErrorAction SilentlyContinue
  try { [Fg.W]::SetForegroundWindow($win.MainWindowHandle) | Out-Null } catch {}
}

Write-Host ''
Write-Host 'Done. Hodos opened with catpicz / bwanq / tackle / bucket as tabs.' -ForegroundColor Green
Write-Host 'Walk: catpicz "Sign in" -> bwanq verify+loan -> tackle member+claim+buy -> bucket list 5% coupon.'
