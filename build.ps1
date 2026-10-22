$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

Push-Location $root
try {
    Write-Host 'Building wasm (release)...'
    cargo build --release --target wasm32-unknown-unknown --lib
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $wasm = Join-Path $root 'target\wasm32-unknown-unknown\release\openkonung.wasm'
    Copy-Item -LiteralPath $wasm -Destination (Join-Path $root 'www\openkonung.wasm') -Force

    $base = (Resolve-Path (Join-Path $root '..')).Path

    $py = Get-Command python -ErrorAction SilentlyContinue
    if (-not $py) { $py = Get-Command py -ErrorAction SilentlyContinue }
    if (-not $py) { throw 'python not found (needed to generate manifest.json)' }

    # 1) Raw manifest (file selection for the packer + fallback metadata).
    $rawManifest = Join-Path $root 'www\manifest.raw.json'
    & $py.Source (Join-Path $root 'tools\make_manifest.py') --base $base --out $rawManifest
    if ($LASTEXITCODE -ne 0) { throw 'make_manifest (raw) failed' }

    # 2) Native packer target: one archive for the whole install.
    Write-Host 'Packing resources (native examples/pack)...'
    $pack = Join-Path $root 'www\openkonung.pack.zip'
    & cargo run --release --example pack -- --base $base --manifest $rawManifest --out $pack
    if ($LASTEXITCODE -ne 0) { throw 'resource pack failed' }

    # 3) Final manifest advertises the pack and keeps the per-file fallback.
    & $py.Source (Join-Path $root 'tools\make_manifest.py') --base $base --pack $pack --out (Join-Path $root 'www\manifest.json')
    if ($LASTEXITCODE -ne 0) { throw 'make_manifest (final) failed' }
    Remove-Item -LiteralPath $rawManifest -Force

    Write-Host 'Built www/openkonung.wasm, www/openkonung.pack.zip and www/manifest.json'
}
finally {
    Pop-Location
}
