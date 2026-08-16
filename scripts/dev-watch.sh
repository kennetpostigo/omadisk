#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"

if ! command -v inotifywait >/dev/null 2>&1; then
  echo "dev-watch: inotifywait not found (install inotify-tools)" >&2
  exit 1
fi

echo "watching ${ROOT} for QML/JS changes (host inotify does not follow the plugin symlink)"
while inotifywait -qq -r -e close_write,create,delete,move \
  --exclude '(/target/|/\.git/)' \
  "${ROOT}"; do
  omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
done
