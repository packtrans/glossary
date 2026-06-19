#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLE="${ROOT}/public/indexes/zh_cn.sample.zip"
TARGET="${ROOT}/public/indexes/zh_cn.zip"

mkdir -p "$(dirname "${TARGET}")"

if [[ -f "${TARGET}" ]]; then
  exit 0
fi

if [[ ! -f "${SAMPLE}" ]]; then
  echo "Missing ${SAMPLE}; run npm run generate-sample-index first." >&2
  exit 1
fi

cp "${SAMPLE}" "${TARGET}"
echo "Copied sample index to ${TARGET}"
