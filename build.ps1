$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

Push-Location $root
try {
    Write-Host 'Building wasm (release)...'
    cargo build --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $wasm = Join-Path $root 'target\wasm32-unknown-unknown\release\openkonung.wasm'
    Copy-Item -LiteralPath $wasm -Destination (Join-Path $root 'www\openkonung.wasm') -Force

    $base = (Resolve-Path (Join-Path $root '..')).Path

    $py = Get-Command python -ErrorAction SilentlyContinue
    if (-not $py) { $py = Get-Command py -ErrorAction SilentlyContinue }
    if (-not $py) { throw 'python not found (needed to generate manifest.json)' }

    & $py.Source (Join-Path $root 'tools\make_manifest.py') --base $base --out (Join-Path $root 'www\manifest.json')

    Write-Host 'Built www/openkonung.wasm and www/manifest.json'
}
finally {
    Pop-Location
}
