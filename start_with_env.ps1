param([string]$EnvFile = ".env")

chcp 65001 > $null
[Console]::OutputEncoding = [Text.UTF8Encoding]::UTF8
$OutputEncoding = [Text.UTF8Encoding]::UTF8

Get-Content $EnvFile |
  Where-Object { $_ -match "^\s*[^#].+=.+$" } |
  ForEach-Object {
    $k,$v = ($_ -split "=", 2)
    [Environment]::SetEnvironmentVariable($k.Trim(), $v.Trim(), "Process")
  }

Write-Host "环境变量已加载，编码 UTF-8"
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass