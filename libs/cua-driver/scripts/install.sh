#!/usr/bin/env bash
# OpenSky Driver: build this fork, never download the upstream Cua product.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec /bin/bash "$SCRIPT_DIR/install-local.sh" "$@"
