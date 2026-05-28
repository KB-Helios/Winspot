$ErrorActionPreference = 'Stop'

Write-Host '== Warm search benchmark =='
rtk cargo run -p winspot-bench --release
