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
  PYTHONPATH=src python3 -m unittest tests.test_tool tests.test_async
)

echo "==> JavaScript SDK tests"
(
  cd "${ROOT_DIR}/klock-js"
  node --test __test__/index.test.mjs
  if [ -d node_modules ]; then
    npm run test:types
  else
    echo "Skipping JS type smoke test because klock-js/node_modules is missing."
  fi
)

echo "==> Website build"
(
  cd "${ROOT_DIR}/../Klock-Website"
  if [ ! -d node_modules ]; then
    echo "Skipping website build because Klock-Website/node_modules is missing."
  elif find node_modules/@next -maxdepth 2 -name 'next-swc.*.node' | grep -q .; then
    npm run build
  else
    echo "Skipping website build because Klock-Website Next.js native dependencies are incomplete."
  fi
)

echo "==> OSS v1 end-to-end demo"
"${ROOT_DIR}/scripts/run_oss_v1_demo.sh"
