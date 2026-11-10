param(
    [string]$Assets = "",
    [switch]$Repack,
    [switch]$NoVideos,
    [switch]$MemPalace
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
Push-Location $root
try {
    Write-Host '==> Checking Rust toolchain...'
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw @'
cargo not found. Install Rust first:
  winget install Rustlang.Rustup
  (or) https://rustup.rs/
Then reopen the terminal and run .\setup.ps1 again.
'@
    }
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if ($rustup) {
        Write-Host '==> Adding wasm32-unknown-unknown target...'
        rustup target add wasm32-unknown-unknown | Out-Null
    }

    Write-Host '==> Checking Python...'
    $py = Get-Command python -ErrorAction SilentlyContinue
    if (-not $py) { $py = Get-Command py -ErrorAction SilentlyContinue }
    if (-not $py) { throw 'python not found (needed for asset discovery and the manifest)' }

    Write-Host '==> Building (first run creates www/konung.zip.pack)...'
    $buildArgs = @{}
    if ($Assets) { $buildArgs['Assets'] = $Assets }
    if ($Repack) { $buildArgs['Repack'] = $true }
    if ($NoVideos) { $buildArgs['NoVideos'] = $true }
    & (Join-Path $root 'build.ps1') @buildArgs

    if ($MemPalace) {
        Write-Host '==> Initializing MemPalace from the repository docs...'
        $mp = Get-Command mempalace -ErrorAction SilentlyContinue
        if (-not $mp) {
            Write-Host 'mempalace CLI not found; skipping. Install it and run:' -ForegroundColor Yellow
            Write-Host '  mempalace init . --palace ~/.mempalace/konung-research --yes --auto-mine --no-llm'
        } else {
            & $mp.Source init . --palace (Join-Path $HOME '.mempalace\konung-research') --yes --auto-mine --no-llm
        }
    }

    Write-Host ''
    Write-Host 'Done. Start the game with:'
    Write-Host '  .\serve.ps1'
    Write-Host 'Then open http://localhost:8000/'
}
finally {
    Pop-Location
}
