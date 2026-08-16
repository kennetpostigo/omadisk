#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
PLUGIN_ID="postman.omadisk"
PLUGIN_LINK="${HOME}/.config/omarchy/plugins/${PLUGIN_ID}"

chmod +x scripts/*.sh
./scripts/build.sh

mkdir -p "${HOME}/.config/omarchy/plugins"
ln -sfn "${ROOT}" "${PLUGIN_LINK}"

omarchy plugin validate "${ROOT}"
omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true

if omarchy plugin list 2>/dev/null | grep -q "${PLUGIN_ID}"; then
  omarchy plugin disable "${PLUGIN_ID}" >/dev/null 2>&1 || true
fi
omarchy plugin enable "${PLUGIN_ID}" --section right

echo
echo "Installed ${PLUGIN_ID} → ${PLUGIN_LINK}"
echo "Summon: omarchy-shell shell summon postman.omadisk '{}'"
echo "QML live-reload: ./scripts/dev-watch.sh"
echo "Menu entry:      ./scripts/install-menu.sh"
