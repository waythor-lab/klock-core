#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${KLOCK_VENV_DIR:-/tmp/klock-crash-test-venv}"
CARGO_TARGET_DIR="${KLOCK_CARGO_TARGET_DIR:-/tmp/klock-target-crash-test}"

section() {
  printf "\n"
  printf "============================================================\n"
  printf "%s\n" "$1"
  printf "============================================================\n"
}

section "KLOCK CRASH TEST"
printf "This demo shows the same repo mutation workflow twice:\n"
printf "1. without Klock: both agents report success, one update disappears\n"
printf "2. with Klock: conflicting writes become GRANT / WAIT / DIE before mutation\n"

section "SETUP"
printf "Virtualenv: %s\n" "${VENV_DIR}"
python3 -m venv --clear "${VENV_DIR}"
"${VENV_DIR}/bin/python" -m ensurepip --upgrade >/dev/null

printf "Installing local Klock packages...\n"
PIP_DISABLE_PIP_VERSION_CHECK=1 \
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
  "${VENV_DIR}/bin/python" -m pip install -q -e "${ROOT_DIR}/klock-py" -e "${ROOT_DIR}/integrations/klock-langchain"

section "1. FAILURE CASE: NO COORDINATION"
"${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/without_klock.py"

section "2. FIXED CASE: KLOCK WAIT-DIE COORDINATION"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/with_klock.py"
)

section "3. CONFLICT TIMELINE: GRANT / WAIT / DIE"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/wait_die_trace.py"
)

section "4. REAL INTEGRATION: LANGCHAIN BASETOOL"
(
  cd "${ROOT_DIR}"
  "${VENV_DIR}/bin/python" "${ROOT_DIR}/examples/oss_v1/langchain_base_tool_demo.py"
)

section "CRASH TEST COMPLETE"
printf "Summary:\n"
printf "%s\n" "- Without Klock: silent overwrite is reproducible."
printf "%s\n" "- With Klock: cooperative agents acquire leases before mutation."
printf "%s\n" "- Conflict handling is visible as GRANT / WAIT / DIE."
printf "%s\n" "- The same protection path works through the LangChain BaseTool adapter."
