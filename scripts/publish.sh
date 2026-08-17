#!/usr/bin/env bash
# Create the public GitHub repo and print the marketplace issue URL.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! gh auth status >/dev/null 2>&1; then
  echo "Not logged in. Run: gh auth login"
  echo "Then re-run: ./scripts/publish.sh"
  exit 1
fi

if ! git remote get-url origin >/dev/null 2>&1; then
  gh repo create kennetpostigo/omadisk \
    --public \
    --source=. \
    --remote=origin \
    --description "DaisyDisk-like disk usage explorer for Omarchy" \
    --push
else
  git push -u origin HEAD
fi

echo
echo "Repo: $(gh repo view --json url -q .url)"
echo
echo "Submit to the Omarchy marketplace:"
echo "https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=submit-plugin.yml"
echo
echo "Suggested form values:"
echo "  Repository URL: https://github.com/kennetpostigo/omadisk"
echo "  Category:       System"
echo "  Tags:           Bar, Quickshell, System"
echo "  Suggested tag:  Disk"
echo "  Notes:          Requires mise/rust to cargo build --release after clone;"
echo "                  omarchy plugin add does not compile. Scanner writes only"
echo "                  under ~/.cache/omadisk/. Read-only explorer."
