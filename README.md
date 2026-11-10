# OpenKonung

A reverse-engineered, browser-playable remake of the 1999 game **Konung**. The
remake reads the original shipped data files directly and reimplements the
behaviour of the original `konung.res` executable. The canonical implementation
is Rust → WebAssembly in `src/`; the browser shell is `www/`.

The original game files are **not** distributed with this repository. You supply
them once; the first build packs them into `www/konung.zip.pack`. After that the
game runs from the pack and never touches a `.res` or `.exe` file.

## Prerequisites

| Tool | Needed for | Check |
|---|---|---|
| Rust + Cargo (`rustup`) | Building the wasm module | `cargo --version` |
| `wasm32-unknown-unknown` target | wasm output | `rustup target add wasm32-unknown-unknown` |
| Python 3.9+ | Asset discovery, manifest, packer helpers | `python --version` |
| Node.js 18+ | Headless smoke tests (optional) | `node --version` |
| ffmpeg | Movie transcoding (optional) | `ffmpeg -version` |
| MemPalace CLI/MCP | Project memory (optional) | `mempalace --version` |

---

## Quick start

### Windows

```powershell
# from the repository root
.\setup.ps1
.\serve.ps1
```

Open <http://localhost:8000/>.

> If PowerShell blocks the script (`running scripts is disabled`), run
> `powershell -ExecutionPolicy Bypass -File .\setup.ps1` instead.

### Linux / macOS

```bash
# from the repository root
bash setup.sh
bash serve.sh
```

Open <http://localhost:8000/>.

`setup` checks Rust, the `wasm32-unknown-unknown` target and Python, then runs
the build. If the original game files are missing it prints exactly which files
it looked for and where to put them.

---

## 1. Where to put the original game files

The build probes **three locations, in this order**:

| # | Location | Notes |
|---|---|---|
| 1 | the repository root (`./`) | Works, but mixes game data with source. |
| 2 | `assets/` inside the repository | **Recommended.** Git-ignored, keeps the repo clean. |
| 3 | the directory above the repository (`../`) | The classic install layout (`web/..` in the original tree). |

The first directory containing every **required** file wins. The build logs each
candidate:

```text
[assets] C:\...\openkonung -> missing: konung.res, GRAPH.RES
[assets] C:\...\openkonung\assets -> OK (8/8 required files)
[assets] using raw install: C:\...\openkonung\assets
```

If nothing is found, the build prints the searched paths, the required file
list, and exits. Copy the files into `assets/` and run it again.

### Required files

| File | Purpose |
|---|---|
| `konung.res` | Game executable tables (code-only; not the item table). |
| `Game.res` | Variant table. |
| `GRAPH.RES` | Terrain, human sprites, overlays, cursors. |
| `Objects.res` | Props and monster animations. |
| `CLIENT.EXE` **or** `LAN_KONG.EXE` | Executable tables. |
| `Game.0`, `Game.1`, `Game.2` | Starting save profiles. |

### Strongly recommended

| File(s) | Purpose |
|---|---|
| `Konung.1` … `Konung.50` | 43 location maps (`Konung.N`). |
| `MENU640R.RES`, `MENU800R.RES` | Main menu screens. |
| `NEWH640R.RES`, `NEWH800R.RES` | New-hero screens. |
| `SAVE640R.RES`, `SAVE800R.RES` | Load/save screens. |
| `INTR640R.RES`, `INTR800R.RES` | In-game HUD / interface sprites. |
| `Quest_r.res` (or `QUEST_R.RES`) | Dialogue and quest script. |
| `SOUNDS.RES` (or `Sounds_.res`) | Sound effects. |
| `FLAMES.RES` | Fire animation frames. |
| `LIGHTS.RES`, `LIGHTS.1` … `LIGHTS.50` | Lighting masks and maps. |
| `music/track*.ogg` | Background music (10 tracks). |

> **Quest file warning:** `Quest_r.res` is the real script. `QUESTR.RES` is a
> placeholder whose dialogues are literal `"???"` stubs. The build prefers
> `QUEST_R.RES`, then `Quest_r.res`, then `QUESTR.RES`.

### Optional

| File(s) | Purpose |
|---|---|
| `Video/*.flm` or `originalCD/Konung/Video/*.flm` | Original movies. The build transcodes them to MP4 if `ffmpeg` is available; otherwise the game skips movies. |
| `KONUNG.CFG` | Original 28-byte settings file. |
| `ListFile.ini`, `Konung.ico` | Launcher metadata. |

---

## 2. Build from zero

### First build

```powershell
# Windows
.\build.ps1
```

```bash
# Linux / macOS
bash build.sh
```

The first build:

1. resolves the original files (`./` → `assets/` → `../`),
2. transcodes the original `.flm` movies to `www/video/*.mp4` when `ffmpeg` is
   available,
3. writes `www/manifest.raw.json`,
4. builds `www/konung.zip.pack` (every resource, profile, map, light map, music
   track and movie),
5. writes `www/manifest.json` and `www/openkonung.wasm`.

### Later builds (pack-only)

If `www/konung.zip.pack` exists, the build **reuses it** and does not read any
`.res`/`.exe` file:

```text
[build] reusing pack: www/konung.zip.pack (pass -Repack to rebuild it)
```

Only the Rust/wasm module and `manifest.json` are rebuilt. This is the normal
workflow while developing the remake.

### Rebuilding the pack

After changing or replacing the original game files:

```powershell
.\build.ps1 -Repack
```

```bash
bash build.sh --repack
```

Useful flags:

| Windows | Bash | Effect |
|---|---|---|
| `-Repack` | `--repack` | Rebuild the pack from the original files. |
| `-NoVideos` | `--no-videos` | Skip movie transcoding. |
| `-Assets <dir>` | `--assets <dir>` | Use an explicit install directory. |

### What the build produces

| Path | Generated? | Notes |
|---|---|---|
| `www/openkonung.wasm` | yes | Rust/wasm module. |
| `www/konung.zip.pack` | yes | Single resource archive. |
| `www/manifest.json` | yes | Shell + pack metadata. |
| `www/manifest.raw.json` | yes | Raw per-file manifest (also embedded in the pack). |
| `www/video/*.mp4` | yes | Transcoded movies. |
| `www/app.js`, `www/index.html`, `www/keymap.js` | no | Hand-written browser shell. |

The pack is ~120 MB, which exceeds GitHub's 100 MB per-file limit, so it is
git-ignored. A fresh clone must rebuild it once from the original files.

---

## 3. Run

```powershell
.\serve.ps1              # Windows
bash serve.sh            # Linux / macOS
```

Then open <http://localhost:8000/>.

`server.py` resolves the same three asset locations automatically. If no raw
files are present it serves `www` only; the pack still works.

### Pack-only guarantee

In a normal run the page requests only:

```text
manifest.json
openkonung.wasm
app.js
index.html
konung.zip.pack
```

No `.res`, `.exe`, `Game.N`, `Konung.N` or `/music/` request is made. The
acceptance test is:

```bash
node tools/pack_smoke.mjs
```

### Development fallback

`?nopack=1` loads the per-file resources instead of the pack. It requires the
raw original files and a manifest generated with `--base`; it is a development
escape hatch, not the normal path.

```text
http://localhost:8000/?nopack=1
```

---

## 4. Verify

Run from the repository root:

```bash
cargo check --all-targets
cargo test                    # dev profile; do NOT use --release
node tools/smoke.mjs
node tools/frontend_smoke.mjs
node tools/pack_smoke.mjs
```

With the server running:

```bash
node tools/page_smoke.mjs http://localhost:8000/
```

The full list of headless smoke tests is in `docs/AGENTS.md`.

---

## 5. Project layout

```text
web/                         git repository root
  README.md                  this file (the only markdown at the root)
  assets/                    original game files (recommended, git-ignored)
  docs/                      all other documentation
  src/                       Rust/wasm source
  www/                       browser shell + generated build artifacts
  tools/                     build scripts, smoke tests, PNG exporters
  examples/                  native helper targets (dump, pack)
  build.ps1  build.sh        build entry points
  setup.ps1  setup.sh        from-scratch bootstrap
  serve.ps1  serve.sh        local HTTP server
  server.py                  static server with asset resolution
```

---

## 6. Documentation

All documentation lives in `docs/`:

| File | Contents |
|---|---|
| `docs/INSTALL.md` | Detailed install / asset / build troubleshooting. |
| `docs/AGENTS.md` | Build conventions, commands, per-subsystem implementation notes. |
| `docs/KONUNG_SOURCE_OF_TRUTH.md` | Binding reverse-engineering contract. |
| `docs/PROJECT.md` | Architecture and workflow. |
| `docs/MODULES.md` | Per-file module map. |
| `docs/DIALOG_SYSTEM.md`, `docs/SOUND_SYSTEM.md`, `docs/TRADE_SYSTEM.md`, `docs/SETTLEMENT_SYSTEM.md`, `docs/DIG_SYSTEM.md`, `docs/CLASS11_ITEMS.md` | Subsystem traces. |
| `docs/CLIENT_EXE_*.md`, `docs/KONUNG_EXE_*.md` | Original executable reverse-engineering notes. |
| `docs/PLAYABLE_V*.md`, `docs/*_FIX*.md` | Chronological development notes (often stale). |
| `docs/MEMPALACE.md` | How to initialize and mine the project memory. |
| `docs/MEMPALACE_DUMP.md` | Full dump of the project MemPalace. |
| `docs/MEMPALACE_COVERAGE.md` | Which memory entries are backed by a doc in this repo. |

---

## 7. MemPalace (project memory)

The project keeps a MemPalace of reverse-engineering notes, subsystem traces and
session diaries. A new contributor can recreate it from this repository.

### Install

Install the MemPalace CLI and MCP server (the standalone binaries or the
`mempalace` package), then verify:

```bash
mempalace --version
```

### Initialize and mine from the docs

```bash
# from the repository root
mempalace init . --palace ~/.mempalace/konung-research --yes --auto-mine --no-llm
```

This creates the palace and mines the repository (respecting `.gitignore`). To
mine only the documentation:

```bash
mempalace mine docs --wing openkonung --agent <your-agent-name>
```

The existing project palace uses two wings:

| Wing | Contents |
|---|---|
| `konung` | Original executable / data-format reverse engineering. |
| `openkonung` | The Rust/wasm remake, packages, settlement system, diaries. |

### Connect the MCP server

Add this to your OpenCode / MCP client configuration (adjust the paths):

```jsonc
{
  "mcp": {
    "mempalace": {
      "type": "local",
      "command": [
        "C:\\Users\\<you>\\.local\\bin\\mempalace-mcp.exe",
        "--palace",
        "C:\\Users\\<you>\\.mempalace\\konung-research"
      ],
      "enabled": true
    }
  }
}
```

On Linux/macOS use the equivalent `mempalace-mcp` path and
`~/.mempalace/konung-research`.

### Dump and coverage

The repository includes a full dump so the knowledge survives even without the
original palace:

- `docs/MEMPALACE_DUMP.md` — human-readable, grouped by wing/room.
- `docs/mempalace_dump.jsonl` — machine-readable, one drawer per line.
- `docs/MEMPALACE_COVERAGE.md` — which drawers are backed by a markdown file in
  this repo, which are backed by another file (CSV/source), and which have no
  repo source.

Regenerate the dump with:

```bash
python tools/dump_mempalace.py --palace ~/.mempalace/konung-research
```

See `docs/MEMPALACE.md` for the full protocol and how to mine the original
install directory into the `konung` wing.

---

## 8. Troubleshooting

### `[assets] ERROR: original Konung files not found`

Copy the original files into `assets/` (recommended). The error lists the
exact required filenames. See `docs/INSTALL.md` for the full table.

### `running scripts is disabled on this system`

Windows blocks `.ps1` files by default. Either run them with a bypass:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
powershell -ExecutionPolicy Bypass -File .\serve.ps1
```

or allow local scripts for your user:

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

### `cargo not found` / `wasm32-unknown-unknown` missing

Install Rust and add the target:

```bash
rustup target add wasm32-unknown-unknown
```

### Movies are missing

The original `.flm` files are AVI/Indeo-5 and need `ffmpeg` to transcode. Install
`ffmpeg` and rebuild with `-Repack`, or accept that movies are skipped.

### The page says the resource pack could not be loaded

Run the first build again. If `www/konung.zip.pack` is missing, the build will
create it from `assets/` (or the other search paths). If you changed the
original files, pass `-Repack`.

### `?nopack=1` returns 404

The per-file fallback needs the raw original files and a manifest generated with
`--base`. Run `.\build.ps1 -Repack` with the files present.

### MemPalace tools are unavailable

Install the CLI/MCP server, then point your client at
`~/.mempalace/konung-research`. The repo dump in `docs/MEMPALACE_DUMP.md` is a
read-only fallback.

---

## 9. Legal

The original Konung game files are the property of their respective rights
holders. This repository contains only the reverse-engineered remake source and
documentation. You must own a legitimate copy of the game to supply the data
files. `assets/` is git-ignored so the original data is not redistributed.
