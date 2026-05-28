$ErrorActionPreference = 'Stop'

Write-Host '== Rust tests =='
rtk cargo test

Write-Host '== WinUI build =='
rtk dotnet build apps/Winspot.App/Winspot.App.csproj -c Debug -p:Platform=x64

Write-Host '== Manual launch check =='
Write-Host '1. Run: rtk cargo run -p winspot-daemon'
Write-Host '2. Run: rtk dotnet run --project apps/Winspot.App/Winspot.App.csproj -c Debug -p:Platform=x64'
Write-Host '3. Verify the Winspot window opens, search is focused, and calc/terminal plus common file/folder names return ranked local results.'
