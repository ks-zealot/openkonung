# Documentation index

The repository root contains only [`README.md`](../README.md). Every other
markdown file lives in this folder.

> Historical notes (files named `*_FIX*.md`, `PLAYABLE_V*.md`, `*_V*.md` and the
> executable reverse-engineering series) often use the old `web/...` path
> prefix. In this repository the root **is** `web/`; treat `web/src` as `src`,
> `web/www` as `www`, and so on.

## Start here

| File | Contents |
|---|---|
| [`../README.md`](../README.md) | Install, asset placement, build, run, MemPalace quick start. |
| [`INSTALL.md`](INSTALL.md) | Detailed install, build flags and troubleshooting. |
| [`AGENTS.md`](AGENTS.md) | Build conventions, commands and per-subsystem implementation notes. |
| [`KONUNG_SOURCE_OF_TRUTH.md`](KONUNG_SOURCE_OF_TRUTH.md) | Binding reverse-engineering contract. |
| [`PROJECT.md`](PROJECT.md) | Architecture and workflow. |
| [`MODULES.md`](MODULES.md) | Per-file module map. |
| [`MEMPALACE.md`](MEMPALACE.md) | Initialize / mine / connect the project memory. |

## Subsystems

| File | Contents |
|---|---|
| [`CLASS11_ITEMS.md`](CLASS11_ITEMS.md) | Class-11 special/quest items. |
| [`CRAFT_SYSTEM.md`](CRAFT_SYSTEM.md) | Alchemy / crafting. |
| [`DIALOG_SYSTEM.md`](DIALOG_SYSTEM.md) | Dialogue and quest script. |
| [`DIG_SYSTEM.md`](DIG_SYSTEM.md) | Buried WorldObjects and the shovel. |
| [`KEY_INPUT_SYSTEM.md`](KEY_INPUT_SYSTEM.md) | Keyboard dispatch. |
| [`KONUNG_SAVE_LOAD.md`](KONUNG_SAVE_LOAD.md) | Save/load formats. |
| [`MOUSE_INPUT_SYSTEM.md`](MOUSE_INPUT_SYSTEM.md) | Mouse dispatch. |
| [`QUEST_OPCODES.md`](QUEST_OPCODES.md) | Quest/dialogue opcode reference. |
| [`SETTLEMENT_SYSTEM.md`](SETTLEMENT_SYSTEM.md) | Settlement pass, workers, roles. |
| [`SETTLEMENT_TRAINING.md`](SETTLEMENT_TRAINING.md) | Voivode/barracks training. |
| [`SOUND_SYSTEM.md`](SOUND_SYSTEM.md) | Sound archive and playback. |
| [`TRADE_SYSTEM.md`](TRADE_SYSTEM.md) | Trade window and merchant stock. |
| [`WORLD_LAYOUT_RECOVERY.md`](WORLD_LAYOUT_RECOVERY.md) | Stitched-world recovery notes. |
| [`WORLD_LAYOUT_TELEPORTS.md`](WORLD_LAYOUT_TELEPORTS.md) | Transition/teleport notes. |

## Original executables

| File | Contents |
|---|---|
| [`CLIENT_EXE_STARTUP_MAIN.md`](CLIENT_EXE_STARTUP_MAIN.md) | `CLIENT.EXE` startup. |
| [`CLIENT_EXE_STATE_MACHINE.md`](CLIENT_EXE_STATE_MACHINE.md) | `CLIENT.EXE` state machine. |
| [`CLIENT_EXE_FUNCTION_MAP.md`](CLIENT_EXE_FUNCTION_MAP.md) | `CLIENT.EXE` function map. |
| [`CLIENT_EXE_BRANCHES.md`](CLIENT_EXE_BRANCHES.md) | `CLIENT.EXE` branches. |
| [`CLIENT_EXE_DATA_FORMATS.md`](CLIENT_EXE_DATA_FORMATS.md) | `CLIENT.EXE` data formats. |
| [`CLIENT_EXE_NETWORK.md`](CLIENT_EXE_NETWORK.md) | `CLIENT.EXE` network. |
| [`CLIENT_EXE_SAVE_LOAD_QUEST.md`](CLIENT_EXE_SAVE_LOAD_QUEST.md) | `CLIENT.EXE` save/load/quest. |
| [`KONUNG_EXE_STARTUP_FORM.md`](KONUNG_EXE_STARTUP_FORM.md) | Launcher form. |
| [`KONUNG_EXE_LAUNCHER.md`](KONUNG_EXE_LAUNCHER.md) | Launcher behaviour. |
| [`KONUNG_EXE_INSTALLER.md`](KONUNG_EXE_INSTALLER.md) | Installer. |
| [`KONUNG_EXE_CD_LAYOUT.md`](KONUNG_EXE_CD_LAYOUT.md) | CD layout. |
| [`KONUNG_EXE_BRANCHES.md`](KONUNG_EXE_BRANCHES.md) | Launcher branches. |
| [`KONUNG_EXE_FUNCTION_MAP.md`](KONUNG_EXE_FUNCTION_MAP.md) | Launcher function map. |
| [`KONUNG_STARTUP_MAIN.md`](KONUNG_STARTUP_MAIN.md) | Game startup. |
| [`KONUNG_STATE_MACHINE.md`](KONUNG_STATE_MACHINE.md) | Game state machine. |
| [`KONUNG_FUNCTION_MAP.md`](KONUNG_FUNCTION_MAP.md) | Game function map. |
| [`KONUNG_GAME2_NPC_REA_REPORT_2026-10-13.md`](KONUNG_GAME2_NPC_REA_REPORT_2026-10-13.md) | Dated NPC REA report. |

## Chronological notes (often stale)

| File | Contents |
|---|---|
| [`PLAYABLE_V13.md`](PLAYABLE_V13.md) … [`PLAYABLE_V15_2.md`](PLAYABLE_V15_2.md) | Playable-prototype milestones. |
| [`PARITY_GAPS_PLAN.md`](PARITY_GAPS_PLAN.md) | Remaining parity work. |
| [`AXES_AND_CLIENT_FIX_V8.md`](AXES_AND_CLIENT_FIX_V8.md) | Axis/client fix. |
| [`CAMERA_ZOOM_FIX.md`](CAMERA_ZOOM_FIX.md) | Camera zoom fix. |
| [`GRAPH_FIT_FIX_V11_1.md`](GRAPH_FIT_FIX_V11_1.md) | Graph fit fix. |
| [`LOCAL_GEOMETRY_FIX_V12.md`](LOCAL_GEOMETRY_FIX_V12.md) | Local geometry fix. |
| [`MAP_DIMENSIONS_FIX.md`](MAP_DIMENSIONS_FIX.md) | Map dimensions fix. |
| [`PERFORMANCE_FIX_V7.md`](PERFORMANCE_FIX_V7.md) | Performance fix. |
| [`V11_2_TELEPORT_UI_FIX.md`](V11_2_TELEPORT_UI_FIX.md) | Teleport UI fix. |
| [`COUNTRY_GRAPH_V10.md`](COUNTRY_GRAPH_V10.md) | Country graph. |
| [`EXACT_GRAPH_V11.md`](EXACT_GRAPH_V11.md) | Exact graph. |
| [`LOCATION_GRAPH_V9.md`](LOCATION_GRAPH_V9.md) | Location graph. |
| [`COMPACT_CONTEXT.md`](COMPACT_CONTEXT.md) | Compact context handoff. |

## Memory

| File | Contents |
|---|---|
| [`MEMPALACE.md`](MEMPALACE.md) | Init / mine / connect. |
| [`MEMPALACE_DUMP.md`](MEMPALACE_DUMP.md) | Full palace dump. |
| [`MEMPALACE_COVERAGE.md`](MEMPALACE_COVERAGE.md) | Coverage report. |
| [`mempalace_dump.jsonl`](mempalace_dump.jsonl) | Machine-readable dump. |
