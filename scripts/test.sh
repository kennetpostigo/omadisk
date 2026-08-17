#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo test =="
if command -v mise >/dev/null 2>&1; then
  mise exec -- cargo test
else
  cargo test
fi

echo "== plugin validate =="
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate .
else
  echo "omarchy not on PATH; skipping plugin validate"
fi

echo "== manifest json =="
python3 -c "import json; json.load(open('manifest.json'))"

echo "== overlay model =="
node tests/overlay_model_test.js

echo "ok"
