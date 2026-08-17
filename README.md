# Omadisk

See what is eating the disk without leaving the Omarchy bar.

A hard-disk icon sits on the bar. Click it and a panel peeks out underneath — the same chrome as Network and Display — with a sunburst map, a folder list, and breadcrumbs. Hover a slice and the matching row lights up. Click either side to drill in.

Plugin id: `postman.omadisk` · License: [MIT](LICENSE) · Kind: bar widget

The UI never walks the filesystem. A small Rust scanner (`omadisk-scan`) streams a capped NDJSON view so the shell stays responsive.

## Install

`omarchy plugin add` clones the plugin. It does not compile the scanner. You need [mise](https://mise.jdx.dev/) (or any Rust toolchain) once, after clone.

```sh
omarchy plugin add https://github.com/kennetpostigo/omadisk.git --enable
cd ~/.config/omarchy/plugins/postman.omadisk
mise install
./scripts/build.sh
omarchy-shell shell rescanPlugins
```

Place the chip if it did not land on the right:

```sh
omarchy plugin enable postman.omadisk --section right
```

Optional Super-menu entry (opt-in; writes only `trigger.omadisk` into your menu extension):

```sh
./scripts/install-menu.sh
```

## Use

Click the disk icon. Escape goes up one folder, or closes the panel at the scan root.

```sh
omarchy-shell shell toggle postman.omadisk
```

| Key | Action |
| --- | --- |
| `j` / `k` / arrows | Move in the list |
| `l` / Enter | Drill into a folder |
| `h` / Backspace | Go up |
| Esc | Go up, or close at the scan root |
| `r` | Rescan |
| `o` | Open the focused path |
| `y` | Copy the focused path |

v1 is **read-only**. There is no delete, trash, or write into the scanned tree.

## Configure

```sh
omarchy bar move postman.omadisk --section right
```

Widget settings (also in the bar widget schema):

| Key | Default | Meaning |
| --- | --- | --- |
| `root` | empty (`$HOME`) | Directory to scan |
| `refreshIntervalSec` | `30` | How often the chip re-reads free space |
| `showFree` | `false` | Show available space next to the icon |

## How it works

- The overlay holds only a **capped depth-3 view** (slices + list rows).
- The scanner measures **allocated size** (`st_blocks * 512`), stays on one filesystem, counts hardlinks once, and does not follow directory symlinks.
- It skips `/proc`, `/dev`, `/sys`, and `/run`.
- The last completed scan is cached under `~/.cache/omadisk/` (`0700` / `0600`) so reopen is instant.
- Protocol: [`protocol.md`](protocol.md).

## Safety

Plugins run **unsandboxed inside `omarchy-shell`**. Review the source before you enable anything.

- The scanner only `stat`s / `scandir`s the chosen root.
- It does not read file contents.
- It does not write into the scanned tree. Cache and session files go only under `~/.cache/omadisk/`.
- That cache lists every path under the scan root — treat it as private.

## Remove

```sh
omarchy plugin disable postman.omadisk
omarchy plugin remove postman.omadisk --yes
rm -rf ~/.cache/omadisk
pkill -f omadisk-scan || true
```

If you added the optional menu trigger, delete the `trigger.omadisk` block from `~/.config/omarchy/extensions/omarchy-menu.jsonc`.

A local symlink install (from `./scripts/dev-install.sh`) is not a git checkout:

```sh
rm -f ~/.config/omarchy/plugins/postman.omadisk
```

## Develop

```sh
mise install
./scripts/test.sh
./scripts/dev-install.sh
./scripts/dev-watch.sh
```

Scanner:

```sh
./target/release/omadisk-scan proto
./target/release/omadisk-scan scan --root "$HOME"
./target/release/omadisk-scan view --root "$HOME"
./target/release/omadisk-scan stat --path "$HOME"
```

## License

MIT. See [LICENSE](LICENSE).
