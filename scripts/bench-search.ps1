$ErrorActionPreference = 'Stop'

Write-Host '== Winspot benchmark suite =='
rtk cargo run -p winspot-bench --release
