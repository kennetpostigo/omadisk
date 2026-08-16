#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mise exec -- cargo build --release
echo "built $(pwd)/target/release/omadisk-scan"
