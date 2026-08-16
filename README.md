# Omadisk

DaisyDisk-like disk-usage explorer for [Omarchy](https://omarchy.org) — a themed overlay (sunburst + child list + breadcrumbs) plus an optional used/free bar chip.

Plugin id: `postman.omadisk`. The overlay never walks the filesystem; a Rust scanner child streams a capped NDJSON view.

## Build

Requires [mise](https://mise.jdx.dev/) (this repo pins `rust = "stable"`).

```bash
mise install
./scripts/build.sh          # mise exec -- cargo build --release
```

`omarchy plugin add` does **not** compile. After clone, run the two commands above so `target/release/omadisk-scan` exists.

## Install (this machine / development)

```bash
./scripts/dev-install.sh    # build, symlink into ~/.config/omarchy/plugins, enable --section right
./scripts/install-menu.sh   # optional Trigger → Disk Usage
./scripts/dev-watch.sh &    # QML live-reload (host inotify does not follow the symlink)
```

Published install:

```bash
git clone <url> && cd omadisk
mise install && ./scripts/build.sh
omarchy plugin add . --enable --yes
```

Enable quirk: a plugin with both `overlay` and `bar-widget` is placed on the bar. If you already enabled an overlay-only copy, a second enable will not insert the chip:

```bash
omarchy plugin disable postman.omadisk
omarchy plugin enable postman.omadisk --section right
```

## Summon

```bash
omarchy-shell shell summon postman.omadisk '{}'
omarchy-shell shell summon postman.omadisk '{"root":"/var"}'
omarchy-shell shell toggle postman.omadisk '{}'
```

Also: Omarchy menu → Trigger → Disk Usage (after `install-menu.sh`), or left-click the bar chip.

Optional Super bind (user file only — check `omarchy menu keybindings --print` first; do not use Super+Shift+D, that is Docker):

```lua
-- ~/.config/hypr/bindings.lua
o.bind("SUPER + SHIFT + U", "Omadisk", "omarchy-shell shell toggle postman.omadisk '{}'")
```

## Keyboard

| Key | Action |
| --- | --- |
| `j` / `k` / arrows | Move in the child list |
| `l` / Enter / Right | Drill into a directory |
| `h` / Left / Backspace | Go up |
| Esc | Go up, or dismiss at the scan root |
| `r` | Rescan the current root |
| `o` | Open the focused path (`xdg-open`) |
| `y` | Copy the focused path (`wl-copy`) |

v1 is **read-only**. There is no delete, trash, or collect.

## Scanner

```
./target/release/omadisk-scan proto
./target/release/omadisk-scan scan --root "$HOME"
./target/release/omadisk-scan view --root "$HOME"
./target/release/omadisk-scan stat --path "$HOME"
```

Defaults: allocated size (`st_blocks * 512`), stay on one filesystem, count hardlinks once, do not follow directory symlinks, skip `/proc /dev /sys /run`. Cache: `~/.cache/omadisk/` (`0700` / `0600`). Protocol: [`protocol.md`](protocol.md).

```bash
mise exec -- cargo test
```

## Security

- Third-party plugins run **unsandboxed inside `omarchy-shell`** (same as every other plugin).
- The scanner is a nicened child of the shell. It only `stat`s / `scandir`s the chosen root and writes under `~/.cache/omadisk/`.
- Cache lists every path under the scan root — treat it as private (`0700`/`0600`).
- `o` / `y` only ever act on a path the walker produced.

## Rollback

```bash
omarchy plugin disable postman.omadisk
rm -f ~/.config/omarchy/plugins/postman.omadisk
# drop trigger.omadisk from ~/.config/omarchy/extensions/omarchy-menu.jsonc
rm -rf ~/.cache/omadisk
pkill -f omadisk-scan || true
```
