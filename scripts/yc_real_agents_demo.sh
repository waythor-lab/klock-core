#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${KLOCK_REAL_AGENTS_VENV_DIR:-/tmp/klock-yc-real-agents-venv}"
CARGO_TARGET_DIR="${KLOCK_REAL_AGENTS_CARGO_TARGET_DIR:-/tmp/klock-target-yc-real-agents}"
SERVER_BIN="${CARGO_TARGET_DIR}/release/klock"

prepare() {
  python3 -m venv --clear "${VENV_DIR}"
  "${VENV_DIR}/bin/python" -m ensurepip --upgrade >/dev/null

  PIP_DISABLE_PIP_VERSION_CHECK=1 \
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
    "${VENV_DIR}/bin/python" -m pip install -q \
      -e "${ROOT_DIR}/klock-py" \
      -e "${ROOT_DIR}/integrations/klock-langchain" \
      "langchain-openai>=0.0.8"

  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
    cargo build --release -q -p klock-cli --manifest-path "${ROOT_DIR}/Cargo.toml"
}

if [[ "${1:-}" == "--prepare" ]]; then
  prepare
  printf "Real-agents YC demo environment prepared.\n"
  printf "Set OPENROUTER_API_KEY, then record:\n"
  printf "  %s\n" "./scripts/yc_real_agents_demo.sh --step"
  exit 0
fi

STEP_MODE="0"
if [[ "${1:-}" == "--step" ]]; then
  STEP_MODE="1"
fi

if [[ ! -x "${VENV_DIR}/bin/python" || ! -x "${SERVER_BIN}" ]]; then
  printf "Preparing real-agents YC demo environment...\n"
  prepare
  printf "\n"
fi

KLOCK_DEMO_STEP="${STEP_MODE}" \
KLOCK_YC_SERVER_BIN="${SERVER_BIN}" \
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/yc_real_agents_demo.py"
