#!/usr/bin/env bash
# Remove only OpenSky Driver, leaving upstream Cua installations alone.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec /bin/bash "$SCRIPT_DIR/uninstall-local.sh" "$@"
