param([int]$Port = 8000)

$root = $PSScriptRoot
$base = (Resolve-Path (Join-Path $root '..')).Path   # original install directory
$www = Join-Path $root 'www'

Write-Host "Open: http://localhost:$Port/"

python (Join-Path $root 'server.py') --port $Port --base $base --www $www
