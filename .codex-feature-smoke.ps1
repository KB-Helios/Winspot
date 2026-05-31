$ErrorActionPreference = 'Stop'

Write-Host '== Stop existing Winspot processes =='
Get-Process Winspot.App,winspot-daemon -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Host '== Start daemon =='
$daemon = Start-Process -FilePath 'rtk' -ArgumentList @('cargo','run','-p','winspot-daemon') -WindowStyle Hidden -PassThru
Start-Sleep -Seconds 2

Write-Host '== Start app =='
$app = Start-Process -FilePath 'rtk' -ArgumentList @('dotnet','run','--project','apps/Winspot.App/Winspot.App.csproj','-c','Debug','-p:Platform=x64') -WindowStyle Hidden -PassThru
Start-Sleep -Seconds 5

Write-Host '== Process responsiveness =='
Get-Process Winspot.App,winspot-daemon -ErrorAction SilentlyContinue |
    Select-Object Id,ProcessName,MainWindowTitle,MainWindowHandle,Responding

Write-Host '== Recent Winspot application errors =='
Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddMinutes(-10)} -ErrorAction SilentlyContinue |
    Where-Object { $_.ProviderName -like '*Application Error*' -or $_.Message -like '*Winspot*' } |
    Select-Object -First 5 TimeCreated,ProviderName,Id,Message |
    Format-List

Write-Host '== Cleanup =='
Get-Process Winspot.App,winspot-daemon -ErrorAction SilentlyContinue | Stop-Process -Force
