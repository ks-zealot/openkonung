param(
    [string]$Assets = "",
    [switch]$Repack,
    [switch]$NoVideos
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

Push-Location $root
try {
    $py = Get-Command python -ErrorAction SilentlyContinue
    if (-not $py) { $py = Get-Command py -ErrorAction SilentlyContinue }
    if (-not $py) { throw 'python not found (needed for asset discovery and manifest generation)' }

    # 1) Locate the original install files (repo root -> assets/ -> parent) or an
    #    existing resource pack. The resolver prints one line per candidate and
    #    returns the raw install directory (empty when only a pack is available).
    $assetArgs = @((Join-Path $root 'tools\assets.py'), '--repo-root', $root, '--print-base')
    if ($Assets) { $assetArgs += @('--assets', $Assets) }
    $base = (& $py.Source @assetArgs | Select-Object -Last 1)
    if ($LASTEXITCODE -ne 0) { throw 'asset discovery failed (see [assets] log above)' }
    $pack = Join-Path $root 'www\konung.zip.pack'
    $legacyPack = Join-Path $root 'www\openkonung.pack.zip'

    # Migrate an existing legacy pack by copying it; never delete the original.
    if (-not (Test-Path -LiteralPath $pack) -and (Test-Path -LiteralPath $legacyPack)) {
        Copy-Item -LiteralPath $legacyPack -Destination $pack
        Write-Host "[build] copied legacy pack to www/konung.zip.pack"
    }
    $havePack = Test-Path -LiteralPath $pack
    $needPack = $Repack -or (-not $havePack)

    if ($needPack -and -not $base) {
        throw 'no original files found; cannot build the resource pack. Put them in web/assets/ or web/ (or pass -Assets <dir>).'
    }

    Write-Host 'Building wasm (release)...'
    cargo build --release --target wasm32-unknown-unknown --lib
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $wasm = Join-Path $root 'target\wasm32-unknown-unknown\release\openkonung.wasm'
    Copy-Item -LiteralPath $wasm -Destination (Join-Path $root 'www\openkonung.wasm') -Force

    if ($needPack) {
        if (-not $NoVideos) {
            Write-Host 'Transcoding videos...'
            & $py.Source (Join-Path $root 'tools\transcode_videos.py') --base $base
            if ($LASTEXITCODE -ne 0) { throw 'video transcode failed' }
        } else {
            Write-Host 'Skipping video transcode (-NoVideos).'
        }

        # Raw manifest: used by the packer and embedded in the archive so later
        # builds can regenerate manifest.json without reading any .res/.exe.
        $rawManifest = Join-Path $root 'www\manifest.raw.json'
        & $py.Source (Join-Path $root 'tools\make_manifest.py') --base $base --out $rawManifest
        if ($LASTEXITCODE -ne 0) { throw 'make_manifest (raw) failed' }

        Write-Host 'Packing resources (native examples/pack)...'
        & cargo run --release --example pack -- --base $base --manifest $rawManifest --out $pack
        if ($LASTEXITCODE -ne 0) { throw 'resource pack failed' }

        & $py.Source (Join-Path $root 'tools\make_manifest.py') --base $base --pack $pack --out (Join-Path $root 'www\manifest.json')
        if ($LASTEXITCODE -ne 0) { throw 'make_manifest (final) failed' }
    } else {
        Write-Host "[build] reusing pack: $pack (pass -Repack to rebuild it)"
        & $py.Source (Join-Path $root 'tools\make_manifest.py') --from-pack $pack --out (Join-Path $root 'www\manifest.json')
        if ($LASTEXITCODE -ne 0) { throw 'make_manifest (from pack) failed' }
    }

    Write-Host 'Built www/openkonung.wasm, www/konung.zip.pack and www/manifest.json'
}
finally {
    Pop-Location
}
