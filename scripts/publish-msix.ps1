param(
    [ValidateSet('win-x64', 'win-arm64')]
    [string]$Runtime = 'win-x64'
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$portableRoot = Join-Path $repoRoot "artifacts\portable\$Runtime"
$msixRoot = Join-Path $repoRoot "artifacts\msix\$Runtime"
$manifest = Join-Path $repoRoot 'packaging\msix\Package.appxmanifest'

rtk powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts\publish-portable.ps1') -Runtime $Runtime

if (Test-Path $msixRoot) {
    Remove-Item -LiteralPath $msixRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $msixRoot | Out-Null
Copy-Item -Path (Join-Path $portableRoot '*') -Destination $msixRoot -Recurse -Force
Copy-Item -LiteralPath $manifest -Destination (Join-Path $msixRoot 'AppxManifest.xml') -Force

$makeAppx = Get-Command MakeAppx.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -eq $makeAppx) {
    $kits = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending
    foreach ($kit in $kits) {
        $candidate = Join-Path $kit.FullName 'x64\MakeAppx.exe'
        if (Test-Path $candidate) {
            $makeAppx = @{ Source = $candidate }
            break
        }
    }
}

if ($null -eq $makeAppx) {
    throw 'MakeAppx.exe was not found. Install the Windows SDK MSIX Packaging Tool component.'
}

$packagePath = Join-Path $repoRoot "artifacts\msix\Winspot-$Runtime.msix"
& $makeAppx.Source pack /d $msixRoot /p $packagePath /overwrite
Write-Host "MSIX package written to $packagePath"
