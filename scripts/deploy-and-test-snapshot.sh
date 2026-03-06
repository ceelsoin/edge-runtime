#!/bin/bash

# Deprecated shim: snapshot format has been removed.
# Usage: ./scripts/deploy-and-test-snapshot.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "⚠️  Snapshot format was removed. Using ESZIP deploy/test flow instead."
exec "$SCRIPT_DIR/deploy-and-test-eszip.sh"
