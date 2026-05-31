param(
    [ValidateSet('win-x64', 'win-arm64')]
    [string]$Runtime = 'win-x64',
    [switch]$PortableMarker
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$publishRoot = Join-Path $repoRoot "artifacts\portable\$Runtime"
$project = Join-Path $repoRoot 'apps\Winspot.App\Winspot.App.csproj'
$platform = if ($Runtime -eq 'win-arm64') { 'ARM64' } else { 'x64' }
$rustTarget = if ($Runtime -eq 'win-arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }

Write-Host "== Publishing Winspot portable ($Runtime) =="
rtk dotnet publish $project -c Release -r $Runtime --self-contained false "-p:Platform=$platform" -o $publishRoot

$daemonProfile = 'release'
$daemon = Join-Path $repoRoot "target\$rustTarget\$daemonProfile\winspot-daemon.exe"
if (!(Test-Path $daemon)) {
    Write-Host '== Building daemon release binary =='
    rtk cargo build -p winspot-daemon --release --target $rustTarget
}

Copy-Item -LiteralPath $daemon -Destination $publishRoot -Force

if ($PortableMarker) {
    New-Item -ItemType File -Force -Path (Join-Path $publishRoot 'Winspot.portable') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $publishRoot 'data') | Out-Null
}

@"
Winspot portable build
Runtime: $Runtime

Run Winspot.App.exe to start the launcher. The bundled winspot-daemon.exe is
started automatically by the app when search first connects.
"@ | Set-Content -LiteralPath (Join-Path $publishRoot 'README.txt')

Write-Host "Portable build written to $publishRoot"
