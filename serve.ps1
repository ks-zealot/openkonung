param(
    [int]$Port = 8000,
    [string]$Base = ""
)

$root = $PSScriptRoot
$www = Join-Path $root 'www'

Write-Host "Open: http://localhost:$Port/"

$argsList = @((Join-Path $root 'server.py'), '--port', $Port, '--www', $www)
if ($Base) { $argsList += @('--base', $Base) }

python @argsList
