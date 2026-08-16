#!/usr/bin/env bash
set -euo pipefail

DEST="${HOME}/.config/omarchy/extensions/omarchy-menu.jsonc"
mkdir -p "$(dirname "${DEST}")"

KEY='"trigger.omadisk"'
BLOCK='  "trigger.omadisk": {
    "icon": "󰋊",
    "label": "Disk Usage",
    "description": "Omadisk — explore disk space",
    "action": "omarchy-shell shell toggle postman.omadisk"
  }'

if [[ ! -f "${DEST}" ]]; then
  printf '{\n%s\n}\n' "${BLOCK}" >"${DEST}"
  echo "wrote ${DEST}"
  exit 0
fi

if grep -q "${KEY}" "${DEST}"; then
  echo "trigger.omadisk already present in ${DEST}"
  exit 0
fi

tmp="$(mktemp)"
# Insert the key before the last closing brace, adding a comma to the previous token if needed.
awk -v block="${BLOCK}" '
  { lines[NR] = $0 }
  END {
    last = NR
    while (last > 0 && lines[last] ~ /^[[:space:]]*$/) last--
    if (last == 0 || lines[last] !~ /^[[:space:]]*}[[:space:]]*$/) {
      for (i = 1; i <= NR; i++) print lines[i]
      print block
      exit
    }
    prev = last - 1
    while (prev > 0 && lines[prev] ~ /^[[:space:]]*$/) prev--
    if (prev > 0 && lines[prev] !~ /,[[:space:]]*$/ && lines[prev] !~ /\{[[:space:]]*$/)
      sub(/[[:space:]]*$/, ",", lines[prev])
    for (i = 1; i < last; i++) print lines[i]
    print block
    print lines[last]
  }
' "${DEST}" >"${tmp}"
mv "${tmp}" "${DEST}"
echo "merged trigger.omadisk into ${DEST}"
