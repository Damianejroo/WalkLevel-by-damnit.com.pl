# Pierwsze uruchomienie (wywolywane przez Uruchom.bat).
# Pobiera przenosne narzedzia do folderu .narzedzia\ - nic nie instaluje w systemie,
# nie wymaga administratora. Usuniecie folderu aplikacji usuwa wszystko.
#
# Wersje i sumy SHA-256 sa przypiete: plik o innej sumie zostanie odrzucony.
# Plik celowo tylko ASCII: PowerShell 5.1 czyta skrypty bez BOM jako ANSI.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # bez paska postepu pobieranie w PS 5.1 jest wielokrotnie szybsze
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.narzedzia'
$repoUrl = 'https://github.com/Damianejroo/bieznia-olek.git'

$packages = @(
    @{
        Name  = 'uv'
        Url   = 'https://github.com/astral-sh/uv/releases/download/0.13.9/uv-x86_64-pc-windows-msvc.zip'
        Sha   = 'edd61f7f5728a56cc62455386661860bdfc7bab65872f39b0c25bdcc1d8e1beb'
        Check = 'uv\uv.exe'
    },
    @{
        Name  = 'git'
        Url   = 'https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/MinGit-2.56.1-64-bit.zip'
        Sha   = '4a92850b6e917da4fd010669b614b55dccf5b0ec333a534cc4be4700dfefdafd'
        Check = 'git\cmd\git.exe'
    }
)

function Install-Tool($pkg) {
    if (Test-Path (Join-Path $tools $pkg.Check)) { return }
    Write-Host "Pobieram $($pkg.Name)..."
    New-Item -ItemType Directory -Force $tools | Out-Null
    $zip = Join-Path $tools "$($pkg.Name).zip"
    $dest = Join-Path $tools $pkg.Name
    Invoke-WebRequest -Uri $pkg.Url -OutFile $zip -UseBasicParsing
    $hash = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne $pkg.Sha) {
        Remove-Item -Force $zip
        throw "Zla suma kontrolna pliku $($pkg.Name) ($hash) - przerwano dla bezpieczenstwa."
    }
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    Expand-Archive -Path $zip -DestinationPath $dest
    Remove-Item -Force $zip
}

function Invoke-Git {
    & (Join-Path $tools 'git\cmd\git.exe') -C $root -c credential.helper= @args
    if ($LASTEXITCODE -ne 0) { throw "git $args nie powiodl sie (kod $LASTEXITCODE)" }
}

try {
    foreach ($pkg in $packages) { Install-Tool $pkg }

    if (-not (Test-Path (Join-Path $root '.git'))) {
        # Folder rozpakowany z ZIP-a: podlaczamy go do repozytorium, zeby dzialaly aktualizacje.
        # Pliki z ZIP-a to te same pliki co w repo, wiec reset niczego nie traci
        # (foldery spoza repo, np. .narzedzia, logs, dane, zostaja nietkniete).
        Write-Host 'Podlaczam folder do aktualizacji z GitHuba...'
        Invoke-Git init -q -b main
        Invoke-Git remote add origin $repoUrl
        Invoke-Git fetch -q --depth=1 origin main
        Invoke-Git reset -q --hard FETCH_HEAD
        Invoke-Git branch -q --set-upstream-to=origin/main main
    }
    Write-Host 'Gotowe.'
    exit 0
}
catch {
    Write-Host ''
    Write-Host "[BLAD] $($_.Exception.Message)"
    Write-Host 'Sprawdz polaczenie z internetem i uruchom ponownie.'
    exit 1
}
