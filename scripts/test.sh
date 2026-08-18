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

echo "== bar model =="
node tests/model_test.js

echo "== qml plaintext =="
python3 - <<'PY'
import re
from pathlib import Path

bad = []
for p in list(Path("overlay").glob("*.qml")) + list(Path("bar").glob("*.qml")):
    lines = p.read_text().splitlines()
    i = 0
    while i < len(lines):
        m = re.match(r"^(\s*)Text\s*\{\s*$", lines[i])
        if m:
            indent = len(m.group(1))
            block = [lines[i]]
            i += 1
            while i < len(lines):
                block.append(lines[i])
                if re.match(r"^" + re.escape(" " * indent) + r"\}\s*$", lines[i]):
                    break
                i += 1
            if "textFormat: Text.PlainText" not in "\n".join(block):
                bad.append(f"{p}:{block[0].strip()}")
        i += 1

if bad:
    raise SystemExit("Text without PlainText:\n  " + "\n  ".join(bad))
print("qml plaintext ok")
PY

echo "ok"
