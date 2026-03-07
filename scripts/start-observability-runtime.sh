#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

COMPOSE_FILE="${REPO_ROOT}/observability/docker-compose.yml"
GRAFANA_URL="http://localhost:3000"
VICTORIAMETRICS_URL="http://localhost:8428/vmui"
PROMETHEUS_URL="http://localhost:9090"
TEMPO_URL="http://localhost:3200"
LOKI_URL="http://localhost:3100"
RUNTIME_HEALTH_URL="http://localhost:9000/_internal/health"

OPEN_ALL_URLS=false

EDGE_RUNTIME_OTEL_ENABLED="${EDGE_RUNTIME_OTEL_ENABLED:-true}"
EDGE_RUNTIME_OTEL_PROTOCOL="${EDGE_RUNTIME_OTEL_PROTOCOL:-http-protobuf}"
EDGE_RUNTIME_OTEL_ENDPOINT="${EDGE_RUNTIME_OTEL_ENDPOINT:-http://127.0.0.1:4318}"
EDGE_RUNTIME_OTEL_SERVICE_NAME="${EDGE_RUNTIME_OTEL_SERVICE_NAME:-deno-edge-runtime-local}"
EDGE_RUNTIME_OTEL_EXPORT_INTERVAL_MS="${EDGE_RUNTIME_OTEL_EXPORT_INTERVAL_MS:-5000}"
EDGE_RUNTIME_OTEL_EXPORT_TIMEOUT_MS="${EDGE_RUNTIME_OTEL_EXPORT_TIMEOUT_MS:-10000}"
EDGE_RUNTIME_OTEL_ENABLE_TRACES="${EDGE_RUNTIME_OTEL_ENABLE_TRACES:-true}"
EDGE_RUNTIME_OTEL_ENABLE_METRICS="${EDGE_RUNTIME_OTEL_ENABLE_METRICS:-true}"
EDGE_RUNTIME_OTEL_ENABLE_LOGS="${EDGE_RUNTIME_OTEL_ENABLE_LOGS:-true}"
EDGE_RUNTIME_OTEL_EXPORT_ISOLATE_LOGS="${EDGE_RUNTIME_OTEL_EXPORT_ISOLATE_LOGS:-true}"
EDGE_RUNTIME_OTEL_ISOLATE_LOG_BATCH_SIZE="${EDGE_RUNTIME_OTEL_ISOLATE_LOG_BATCH_SIZE:-256}"

usage() {
  cat <<'EOF'
Usage: start-observability-runtime.sh [--all]

Options:
  --all   Open all main observability UIs in browser (Grafana, VictoriaMetrics, Prometheus, Tempo, Loki)
  -h      Show this help message
  --help  Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      OPEN_ALL_URLS=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[error] unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

open_url() {
  local url="$1"
  if command -v open >/dev/null 2>&1; then
    open "$url" >/dev/null 2>&1 || true
    return
  fi

  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$url" >/dev/null 2>&1 || true
    return
  fi

  echo "[warn] no browser opener found (open/xdg-open). URL: $url"
}

wait_http_ok() {
  local url="$1"
  local max_attempts="${2:-60}"
  local sleep_secs="${3:-1}"

  local attempt=1
  while (( attempt <= max_attempts )); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi

    sleep "$sleep_secs"
    attempt=$((attempt + 1))
  done

  return 1
}

cleanup() {
  if [[ -n "${RUNTIME_PID:-}" ]] && kill -0 "${RUNTIME_PID}" >/dev/null 2>&1; then
    echo "[info] stopping edge runtime (pid ${RUNTIME_PID})"
    kill "${RUNTIME_PID}" >/dev/null 2>&1 || true
    wait "${RUNTIME_PID}" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

echo "[info] starting observability stack"
cd "${REPO_ROOT}"
docker compose -f "${COMPOSE_FILE}" up -d

if wait_http_ok "${GRAFANA_URL}" 90 1; then
  echo "[info] grafana is ready at ${GRAFANA_URL}"
else
  echo "[warn] grafana did not become ready in time: ${GRAFANA_URL}"
fi

echo "[info] starting edge runtime with OpenTelemetry"
EDGE_RUNTIME_OTEL_ENABLED="${EDGE_RUNTIME_OTEL_ENABLED}" \
EDGE_RUNTIME_OTEL_PROTOCOL="${EDGE_RUNTIME_OTEL_PROTOCOL}" \
EDGE_RUNTIME_OTEL_ENDPOINT="${EDGE_RUNTIME_OTEL_ENDPOINT}" \
EDGE_RUNTIME_OTEL_SERVICE_NAME="${EDGE_RUNTIME_OTEL_SERVICE_NAME}" \
EDGE_RUNTIME_OTEL_EXPORT_INTERVAL_MS="${EDGE_RUNTIME_OTEL_EXPORT_INTERVAL_MS}" \
EDGE_RUNTIME_OTEL_EXPORT_TIMEOUT_MS="${EDGE_RUNTIME_OTEL_EXPORT_TIMEOUT_MS}" \
EDGE_RUNTIME_OTEL_ENABLE_TRACES="${EDGE_RUNTIME_OTEL_ENABLE_TRACES}" \
EDGE_RUNTIME_OTEL_ENABLE_METRICS="${EDGE_RUNTIME_OTEL_ENABLE_METRICS}" \
EDGE_RUNTIME_OTEL_ENABLE_LOGS="${EDGE_RUNTIME_OTEL_ENABLE_LOGS}" \
EDGE_RUNTIME_OTEL_EXPORT_ISOLATE_LOGS="${EDGE_RUNTIME_OTEL_EXPORT_ISOLATE_LOGS}" \
EDGE_RUNTIME_OTEL_ISOLATE_LOG_BATCH_SIZE="${EDGE_RUNTIME_OTEL_ISOLATE_LOG_BATCH_SIZE}" \
cargo run -- start --print-isolate-logs false &

RUNTIME_PID="$!"

if wait_http_ok "${RUNTIME_HEALTH_URL}" 120 1; then
  echo "[info] edge runtime is ready at ${RUNTIME_HEALTH_URL}"
else
  echo "[warn] edge runtime health check timed out: ${RUNTIME_HEALTH_URL}"
fi

echo "[info] opening grafana"
open_url "${GRAFANA_URL}"

if [[ "${OPEN_ALL_URLS}" == "true" ]]; then
  echo "[info] opening all observability UIs"
  open_url "${VICTORIAMETRICS_URL}"
  open_url "${PROMETHEUS_URL}"
  open_url "${TEMPO_URL}"
  open_url "${LOKI_URL}"
fi

echo "[info] runtime pid: ${RUNTIME_PID}"
echo "[info] press Ctrl+C to stop runtime (observability containers stay up)"

wait "${RUNTIME_PID}"
