#!/bin/bash

# Deprecated shim: snapshot format has been removed.
# Usage: ./scripts/bundle-snapshot.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "⚠️  Snapshot format was removed. Using ESZIP bundling instead."
exec "$SCRIPT_DIR/bundle-eszip.sh"
