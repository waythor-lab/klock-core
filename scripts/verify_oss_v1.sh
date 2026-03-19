#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Rust checks"
(
  cd "${ROOT_DIR}"
  cargo check -p klock-py -p klock-cli -p klock-core
)

echo "==> LangChain integration tests"
(
  cd "${ROOT_DIR}/integrations/klock-langchain"
  PYTHONPATH=src python3 -m unittest tests.test_tool
)

echo "==> JavaScript SDK tests"
(
  cd "${ROOT_DIR}/klock-js"
  node __test__/index.test.mjs
)

echo "==> Website build"
(
  cd "${ROOT_DIR}/../Klock-Website"
  npm run build
)

echo "==> OSS v1 end-to-end demo"
"${ROOT_DIR}/scripts/run_oss_v1_demo.sh"
