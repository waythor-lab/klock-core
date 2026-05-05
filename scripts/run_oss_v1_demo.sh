#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${KLOCK_VENV_DIR:-/tmp/klock-oss-v1-venv}"

echo "==> Creating demo virtualenv at ${VENV_DIR}"
python3 -m venv --clear "${VENV_DIR}"
"${VENV_DIR}/bin/python" -m ensurepip --upgrade >/dev/null

echo "==> Installing local Klock packages"
PIP_DISABLE_PIP_VERSION_CHECK=1 \
CARGO_TARGET_DIR="${KLOCK_CARGO_TARGET_DIR:-/tmp/klock-target-oss-v1}" \
  "${VENV_DIR}/bin/python" -m pip install -e "${ROOT_DIR}/klock-py" -e "${ROOT_DIR}/integrations/klock-langchain"

echo "==> Running Python SDK facade tests"
"${VENV_DIR}/bin/python" -m unittest discover -s "${ROOT_DIR}/klock-py/tests"

echo "==> Running unprotected repro"
"${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/without_klock.py"

echo "==> Running coordinated repro (auto-starts local Klock server if needed)"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/with_klock.py"
)

echo "==> Running WAIT-DIE trace"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/wait_die_trace.py"
)

echo "==> Running LangChain BaseTool demo"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/langchain_base_tool_demo.py"
)

echo
echo "OSS v1 demo completed."
