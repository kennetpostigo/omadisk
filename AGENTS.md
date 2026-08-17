# AGENTS.md

Instructions for coding agents working in this repository. Humans should start with [README.md](README.md). The scanner contract is [protocol.md](protocol.md).

## What this is

Omadisk (`postman.omadisk`) is a third-party Omarchy **bar-widget** plugin: a disk icon on the bar opens a KeyboardPanel overlay (sunburst + list + breadcrumbs). The overlay is read-only. A Rust child, `omadisk-scan`, walks the disk and speaks NDJSON on stdout.

Do not turn this into a standalone app, an Electron/Tamagui UI, or a first-party `omarchy.*` plugin.

## Hard constraints

- **Never walk the filesystem from QML/JS.** No `FolderListModel`, no recursive `XMLHttpRequest` of directories, no shelling out to `du`/`find` from the overlay. All walks go through `omadisk-scan`.
- **Never write into the scanned tree.** Cache and session files go only under `$XDG_CACHE_HOME/omadisk` or `~/.cache/omadisk` (`0700` dirs, `0600` files).
- **Never edit `/usr/share/omarchy/`.** User plugin code lives here and, when installed, under `~/.config/omarchy/plugins/postman.omadisk/`.
- **Do not change the plugin id** (`postman.omadisk`) unless the user explicitly asks. It is the IPC target, the bar entry, the menu action, and the marketplace identity.
- **Do not add delete, trash, or chmod of scanned files.** v1 is read-only.
- **Do not commit** `DESIGN.md`, `PLAN.md`, scan caches, `session.json`, `.env`, or `target/`.
- **`omarchy plugin add` does not compile.** After clone, `mise install && ./scripts/build.sh` must produce `target/release/omadisk-scan`. The overlay resolves that path from `manifest.__sourceDir` or `Qt.resolvedUrl("../target/release/omadisk-scan")`.

## Layout

```
manifest.json           id, kinds: ["bar-widget"], settings schema
bar/BarWidget.qml       Panel + BarIconButton + KeyboardPanel host
bar/Model.js            chip icon, tooltip, stat parse
overlay/Overlay.qml     session, Process children, drill, keyboard
overlay/OverlayModel.js layoutSlices, hitTestSlices, project, parseLine
overlay/SunburstCanvas.qml
overlay/ChildList.qml
overlay/BreadcrumbBar.qml
overlay/HubLabel.qml
overlay/StatusBar.qml
overlay/Format.js
src/main.rs             scan | view | stat | proto
src/walk.rs             iterative walk
src/view.rs             depth-3 projection + Other collapse
src/cache.rs            atomic publish, 4-slot eviction
src/statfs.rs           statvfs
src/protocol.rs         NDJSON events
tests/                  cargo unit + CLI goldens
scripts/build.sh
scripts/test.sh         the verification gate
scripts/dev-install.sh  symlink into ~/.config/omarchy/plugins
```

## Build and verify

```sh
mise install
./scripts/test.sh          # cargo test + omarchy plugin validate + manifest JSON
./scripts/build.sh         # release scanner the overlay actually launches
./scripts/dev-install.sh   # symlink + enable on the right
```

`scripts/test.sh` must stay green. It currently expects 44 unit tests and 12 CLI tests. If you change the protocol or view shape, update `tests/goldens/` and the parsers in `protocol.rs` / `OverlayModel.js` together.

After QML edits on a **symlink** install, saved files under this repo are the plugin tree, but the shell's inotify watcher does not follow the symlink. Force a reload with `omarchy-shell shell rescanPlugins` or `omarchy restart shell` if the overlay looks stale.

## Protocol invariants

Full schema: [protocol.md](protocol.md).

- One JSON object per stdout line. `v` must be `1`. Unknown `type` → ignore.
- stderr is human logs only. The overlay must not parse it as events.
- `scan` publishes the cache only on a clean finish. SIGINT/SIGTERM/abort → no `tree.json` replace.
- `view` exit `3` is cache miss (start a scan). Exit `2` is missing root. Exit `0` is one `view` event.
- View payload is capped: depth 3, ≤40 slices/ring + Other, ≤120 slice nodes, ≤200 list rows. Overlay **drops** a `view` with more than 120 slice nodes.
- Other wedges use the synthetic path `parent + "/\0other"` and are not drillable. Unix paths cannot contain NUL, so this cannot collide.

## Overlay invariants

These are easy to regress. If you touch hover, click, breadcrumbs, or session, re-check all four.

1. **Shared hover.** One `hoverPath` plus `hoverTick`. Sunburst and list both read live state. Do **not** clear `hoverPath` in `onExited` on the canvas — crossing into the list would flash empty. Dim non-hovered slices; do not hide them.
2. **Slice ↔ list.** Clicking a slice selects and drills the same path as the list row. `hitTestSlices` must match `layoutSlices` geometry (`startDeg`, `sweepDeg`, `innerR`, `outerR`).
3. **Breadcrumbs.** Ancestors of `focusPath` are drillable even when they are not in `listRows`. `isDrillable` must treat the scan root and ancestors as folders when `pathKind` is empty.
4. **Session.** `persistSession` updates `sessionObj` in memory **and** writes `session.json` on close. `startSession` must not reset `focusPath` to `scanRoot` when a valid descendant is stored. FileView's `sessionObj` can be stale — memory wins if it is newer.

Do not put a role named `color` on a QML `ListModel`. Quickshell reserves it and the sunburst goes monochrome. Folder fills live on the laid-out slice objects from `layoutSlices`.

## Scanner invariants

- Default metric is **allocated** (`st_blocks * 512`), not apparent size.
- Stay on one filesystem. Do not follow directory symlinks. Count hardlinks once unless `--count-hardlinks`.
- Always skip `/proc`, `/dev`, `/sys`, `/run`.
- Launch scans via `niceIoniceConcat` (`nice -n 10`, `ionice -c 2 -n 7`). If `ionice` is missing (exit 127), retry without it and remember `ioniceAvailable = false`.
- The walk must not mutate the source tree. CLI tests assert this.

## What good changes look like

- Small, local diffs. Match surrounding QML/Rust style. No new frameworks.
- Theme through `qs.Commons.Color` / `Style` / `Border`. Chrome follows the existing KeyboardPanel, not a custom window.
- New scanner behavior = protocol event or flag, plus a test. Do not grow the QML-held tree.
- Comments only for non-obvious constraints (NUL Other path, ListModel `color`, session memory vs FileView).

## Out of scope unless asked

- Volume picker, disk-relative free wedge, delete/trash, multi-root compare.
- Rewriting the scanner in another language.
- Packaging a prebuilt `omadisk-scan` for every arch.
- Changing marketplace id, license, or install/remove docs without updating README and `manifest.json` together.
