# MemPalace — project memory

OpenKonung keeps its reverse-engineering notes, subsystem traces and session
diaries in a [MemPalace](https://github.com/mempalace/mempalace). A new
contributor can recreate the palace from this repository.

The project uses two wings:

| Wing | Contents |
|---|---|
| `konung` | Original executable / data-format reverse engineering. |
| `openkonung` | The Rust/wasm remake, packages, settlement system, diaries. |

## 1. Install

Install the MemPalace CLI and MCP server (standalone binaries or the
`mempalace` package). Verify:

```bash
mempalace --version
mempalace-mcp --help
```

## 2. Initialize from the repository

```bash
cd web
mempalace init . --palace ~/.mempalace/konung-research --yes --auto-mine --no-llm
```

This creates the palace and mines the repository. The miner respects
`.gitignore`, so `target/`, `assets/`, `www/konung.zip.pack` and the transcoded
movies are skipped.

### Mine only the docs

```bash
mempalace mine docs --wing openkonung --agent <your-agent-name>
```

### Mine the original reverse-engineering notes

If the original install directory is available, mine its notes too:

```bash
mempalace mine /path/to/original_install --wing konung --agent <your-agent-name>
```

## 3. Connect the MCP server

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

## 4. What is in the palace

The dump in this repository records the current state:

| File | Contents |
|---|---|
| `MEMPALACE_DUMP.md` | Human-readable dump, grouped by wing/room. |
| `mempalace_dump.jsonl` | One logical drawer per line (machine-readable). |
| `MEMPALACE_COVERAGE.md` | Which drawers are backed by a repo file, which are missing, and which have no source. |

The coverage report is the quickest way to see what is **not** in a markdown
file. Typical categories:

- **markdown** — the drawer came from a doc that is in this repo.
- **other repo file** — the drawer came from a CSV/source file in this repo.
- **missing** — the drawer names a markdown file that is not in this repo.
- **no source** — diary entries, package notes and session memory.

## 5. Regenerate the dump

From the repository root:

```bash
python tools/dump_mempalace.py
```

Or with an explicit palace:

```bash
python tools/dump_mempalace.py --palace ~/.mempalace/konung-research
python tools/dump_mempalace.py --palace /path/to/chroma.sqlite3 --out-dir docs
```

The script reads `chroma.sqlite3` directly with Python's `sqlite3` module; it
does not require the `mempalace` Python package.

## 6. Protocol

The palace expects the standard MemPalace protocol:

1. call `mempalace_status` on wake-up;
2. query the palace before answering questions about past work;
3. write a diary entry after each session;
4. use `mempalace_kg_supersede` when a single-valued fact changes.

See the MemPalace documentation for the full AAAK dialect and tool list.
