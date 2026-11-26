# Budowanie WalkLevel: interfejs, exe (release, statyczny runtime C++) i instalator Inno Setup.
#   powershell -ExecutionPolicy Bypass -File walklevel\packaging\zbuduj.ps1 -Wersja 0.6.0
# Wynik: walklevel\dist-instalator\Bieznia-Setup-<wersja>.exe + .sha256 (jak w 0.5.x).
param(
    [Parameter(Mandatory = $true)][string]$Wersja,
    [string]$Iscc = ""
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot            # walklevel\
Set-Location $root

if ($Wersja -notmatch '^\d+\.\d+\.\d+$') { throw "Wersja musi mieć postać 1.2.3" }

# 1. interfejs (dist/) - Tauri wbudowuje go w exe
npm run check; if ($LASTEXITCODE) { throw "npm run check" }
npm run build; if ($LASTEXITCODE) { throw "npm run build" }

# 2. exe: osobny katalog kompilacji wydania, runtime C++ statycznie (bez vcruntime140.dll)
$env:CARGO_TARGET_DIR = Join-Path $root 'target-release'
$env:RUSTFLAGS = '-C target-feature=+crt-static'
# custom-protocol: interfejs z dist/ wbudowany w exe (bez tego exe szuka serwera deweloperskiego
# Vite i okno nigdy się nie pokazuje) - to samo, co włącza `tauri build`
cargo build --release -p walklevel --locked --features tauri/custom-protocol; if ($LASTEXITCODE) { throw "cargo build" }
Remove-Item Env:RUSTFLAGS

# 3. instalator
if (-not $Iscc) { $Iscc = Join-Path $root 'narzedzia\inno\ISCC.exe' }
if (-not (Test-Path $Iscc)) { throw "Brak Inno Setup: $Iscc" }
& $Iscc "/DAppVersion=$Wersja" (Join-Path $PSScriptRoot 'instalator.iss'); if ($LASTEXITCODE) { throw "ISCC" }

$setup = Join-Path $root "dist-instalator\Bieznia-Setup-$Wersja.exe"
$hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLower()
"$hash  Bieznia-Setup-$Wersja.exe" | Out-File -Encoding ascii "$setup.sha256"
$mb = [math]::Round((Get-Item $setup).Length / 1MB, 1)
$exe = [math]::Round((Get-Item (Join-Path $root 'target-release\release\walklevel.exe')).Length / 1MB, 1)
"Gotowe: $setup ($mb MB, exe $exe MB), SHA-256 $hash"
