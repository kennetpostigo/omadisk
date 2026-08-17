# Omadisk

A DaisyDisk-like disk usage explorer for [Omarchy](https://omarchy.org). A hard-disk icon on the bar opens a panel that peeks out underneath — sunburst map, folder list, breadcrumbs — using the same chrome as Network and Display.

Plugin id: `postman.omadisk`. The UI never walks the filesystem. A Rust scanner child streams a capped NDJSON view.

## Install

Needs [mise](https://mise.jdx.dev/) (or a Rust toolchain) so the scanner can be built. `omarchy plugin add` does not compile.

```sh
git clone https://github.com/kennetpostigo/omadisk.git
cd omadisk
mise install
./scripts/build.sh
omarchy plugin add "$(pwd)" --enable --yes
```

Or after the repo is public:

```sh
omarchy plugin add https://github.com/kennetpostigo/omadisk.git --enable
cd ~/.config/omarchy/plugins/postman.omadisk
mise install && ./scripts/build.sh
omarchy-shell shell rescanPlugins
```

Place the chip:

```sh
omarchy plugin enable postman.omadisk --section right
```

Optional menu entry:

```sh
./scripts/install-menu.sh
```

## Usage

Click the disk icon on the bar. Escape closes (or goes up one folder first).

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

v1 is **read-only**. There is no delete or trash.

## Configure

```sh
omarchy bar move postman.omadisk --section right
```

Optional scan root (empty = `$HOME`) via the widget settings schema `root`.

## Remove

```sh
omarchy plugin disable postman.omadisk
omarchy plugin remove postman.omadisk --yes
# if you used a local symlink instead:
rm -f ~/.config/omarchy/plugins/postman.omadisk
# optional: drop trigger.omadisk from ~/.config/omarchy/extensions/omarchy-menu.jsonc
rm -rf ~/.cache/omadisk
pkill -f omadisk-scan || true
```

## Development

```sh
mise install
./scripts/test.sh
./scripts/dev-install.sh
./scripts/dev-watch.sh &
```

Scanner:

```
./target/release/omadisk-scan proto
./target/release/omadisk-scan scan --root "$HOME"
./target/release/omadisk-scan view --root "$HOME"
./target/release/omadisk-scan stat --path "$HOME"
```

Defaults: allocated size (`st_blocks * 512`), stay on one filesystem, count hardlinks once, do not follow directory symlinks, skip `/proc /dev /sys /run`. Cache: `~/.cache/omadisk/` (`0700` / `0600`). Protocol: [`protocol.md`](protocol.md).

## Security

- Plugins run **unsandboxed inside `omarchy-shell`**.
- The scanner only `stat`s / `scandir`s the chosen root and writes under `~/.cache/omadisk/`.
- It does not read file contents or write into the scanned tree.
- Cache lists every path under the scan root — treat it as private.

## License

MIT. See [LICENSE](LICENSE).
