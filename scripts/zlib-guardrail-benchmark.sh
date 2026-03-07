#!/bin/bash

# Zlib guardrail benchmark runner
# Focuses on node:zlib compatibility checks that exercise native limits.
# Usage: ./scripts/zlib-guardrail-benchmark.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "[zlib-guardrail] Running focused compatibility check..."
START_TIME=$(date +%s)

cargo test -p functions --test node_module_imports additional_node_stub_modules_import_and_behave_predictably -- --nocapture

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo "[zlib-guardrail] Completed in ${ELAPSED}s"
echo "[zlib-guardrail] This check validates:"
echo "  - async/sync one-shot zlib compatibility"
echo "  - maxOutputLength enforcement"
echo "  - hard max input size enforcement"
echo "  - operationTimeoutMs option validation"
