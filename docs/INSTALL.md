# Install from zero

This guide takes a fresh clone to a running browser build with the fewest
commands. The original Konung data files are not included; you supply them once.

## 0. Prerequisites

| Tool | Why | Check |
|---|---|---|
| Rust + Cargo | Builds the wasm module. | `cargo --version` |
| `wasm32-unknown-unknown` target | wasm output. | `rustup target list --installed` |
| Python 3.9+ | Asset discovery, manifest, packer helper scripts. | `python --version` |
| Node.js 18+ | Headless smoke tests (optional). | `node --version` |
| ffmpeg | Transcodes the original `.flm` movies (optional). | `ffmpeg -version` |
| MemPalace CLI/MCP | Project memory (optional but recommended). | `mempalace --version` |

Add the wasm target once:

```bash
rustup target add wasm32-unknown-unknown
```

## 1. Put the original files in place

The build probes three directories, in order:

1. the repository root (`./`)
2. `assets/` inside the repository — **recommended**
3. the directory above the repository (`../`)

The first directory that contains every required file is used. The build prints
one line per candidate and a final error with the missing names if none match.

```text
[assets] C:\...\openkonung -> missing: konung.res, GRAPH.RES
[assets] C:\...\openkonung\assets -> OK (8/8 required files)
[assets] using raw install: C:\...\openkonung\assets
```

### Required files

```text
konung.res
Game.res
GRAPH.RES
Objects.res
CLIENT.EXE          (or LAN_KONG.EXE)
Game.0
Game.1
Game.2
```

### Recommended files

```text
Konung.1 .. Konung.50
MENU640R.RES  MENU800R.RES
NEWH640R.RES  NEWH800R.RES
SAVE640R.RES  SAVE800R.RES
INTR640R.RES  INTR800R.RES
Quest_r.res          (or QUEST_R.RES; not the QUESTR.RES placeholder)
SOUNDS.RES           (or Sounds_.res)
FLAMES.RES
LIGHTS.RES  LIGHTS.1 .. LIGHTS.50
music/track*.ogg
```

### Optional files

```text
Video/*.flm                       (or originalCD/Konung/Video/*.flm)
KONUNG.CFG
ListFile.ini
Konung.ico
```

`assets/` is git-ignored, so the original data is never committed.

## 2. Build

### Windows

```powershell
# from the repository root
.\setup.ps1
```

`setup.ps1` checks Rust and Python, adds the wasm target, and calls
`build.ps1`. Use `.\build.ps1` directly for later builds.

### Linux / macOS

```bash
# from the repository root
bash setup.sh
```

`setup.sh` installs the base packages and Rust via rustup if needed, then calls
`build.sh`. Use `bash build.sh` directly for later builds.

### First build vs later builds

| Situation | Command | Reads `.res`/`.exe`? |
|---|---|---|
| No pack yet | `build.ps1` / `build.sh` | Yes, once. |
| Pack exists | `build.ps1` / `build.sh` | No, reuses the pack. |
| Original files changed | `build.ps1 -Repack` / `build.sh --repack` | Yes, rebuilds the pack. |
| Movies unavailable | `build.ps1 -NoVideos` / `build.sh --no-videos` | Yes, skips transcoding. |
| Files in a custom dir | `build.ps1 -Assets D:\Konung` / `build.sh --assets /mnt/konung` | Yes. |

### Outputs

```text
www/openkonung.wasm       wasm module
www/konung.zip.pack       all original resources + profiles + maps + music + movies
www/manifest.json         shell + pack metadata
www/manifest.raw.json     raw per-file manifest (also embedded in the pack)
www/video/*.mp4           transcoded movies
```

## 3. Run

```powershell
.\serve.ps1
```

```bash
bash serve.sh
```

Open <http://localhost:8000/>. The server auto-detects the same three asset
locations. If no raw files are present it serves `www` only and the pack still
works.

### Pack-only mode

The normal page loads only:

```text
manifest.json
openkonung.wasm
app.js
index.html
konung.zip.pack
```

It does not request `.res`, `.exe`, `Game.N`, `Konung.N` or `/music/`.

The development fallback is `?nopack=1`; it needs the raw files and a manifest
built with `--base`.

## 4. Verify

```bash
cargo check --all-targets
cargo test
node tools/smoke.mjs
node tools/frontend_smoke.mjs
node tools/pack_smoke.mjs
```

With the server running:

```bash
node tools/page_smoke.mjs http://localhost:8000/
```

## 5. Common problems

### `cargo not found`

Install Rust from <https://rustup.rs/> and reopen the terminal. On Windows:
`winget install Rustlang.Rustup`.

### `running scripts is disabled on this system`

Windows blocks `.ps1` files by default. Run them with:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

or allow local scripts for your user:

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

### `error: failed to find a precompiled artifact for wasm32-unknown-unknown`

```bash
rustup target add wasm32-unknown-unknown
```

### `[assets] ERROR: original Konung files not found`

Put the required files in `assets/`. The error lists every searched
directory and every missing filename.

### The page says the resource pack could not be loaded

`www/konung.zip.pack` is missing or corrupt. Run `build.ps1 -Repack` (Windows)
or `build.sh --repack` with the original files present.

### Movies are skipped

Install `ffmpeg`, then rebuild with `-Repack` / `--repack`. The build never
deletes the previous MP4s; ffmpeg overwrites them in place.

### `?nopack=1` returns 404

The fallback needs the raw original files and a manifest generated with
`--base`. Rebuild with `-Repack` while the files are present.

### The pack is ~120 MB and git will not accept it

GitHub rejects files over 100 MB. The pack is git-ignored; each clone rebuilds
it once from the original files.

### MemPalace is empty

See `MEMPALACE.md`:

```bash
mempalace init . --palace ~/.mempalace/konung-research --yes --auto-mine --no-llm
```
