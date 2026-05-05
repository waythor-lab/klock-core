#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${KLOCK_YC_VENV_DIR:-/tmp/klock-yc-demo-venv}"
CARGO_TARGET_DIR="${KLOCK_YC_CARGO_TARGET_DIR:-/tmp/klock-target-yc-demo}"
SERVER_BIN="${CARGO_TARGET_DIR}/release/klock"

prepare() {
  python3 -m venv --clear "${VENV_DIR}"
  "${VENV_DIR}/bin/python" -m ensurepip --upgrade >/dev/null

  PIP_DISABLE_PIP_VERSION_CHECK=1 \
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
    "${VENV_DIR}/bin/python" -m pip install -q -e "${ROOT_DIR}/klock-py" -e "${ROOT_DIR}/integrations/klock-langchain"

  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
    cargo build --release -q -p klock-cli --manifest-path "${ROOT_DIR}/Cargo.toml"
}

if [[ "${1:-}" == "--prepare" ]]; then
  prepare
  printf "YC demo environment prepared.\n"
  printf "Record this command next:\n"
  printf "  %s\n" "./scripts/yc_demo.sh --step"
  exit 0
fi

STEP_MODE="0"
if [[ "${1:-}" == "--step" ]]; then
  STEP_MODE="1"
fi

if [[ ! -x "${VENV_DIR}/bin/python" || ! -x "${SERVER_BIN}" ]]; then
  printf "Preparing YC demo environment...\n"
  prepare
  printf "\n"
fi

KLOCK_DEMO_STEP="${STEP_MODE}" \
KLOCK_YC_SERVER_BIN="${SERVER_BIN}" \
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/yc_demo.py"
