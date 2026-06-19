#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT="${ROOT}/web/public/indexes/zh_cn.sample.zip"

mkdir -p "$(dirname "${OUTPUT}")"
cargo test -p packtrans-glossary-wasm tests::write_sample_index_zip -- --ignored --exact

if [[ ! -f "${OUTPUT}" ]]; then
  echo "sample index was not written to ${OUTPUT}" >&2
  exit 1
fi

echo "Wrote ${OUTPUT}"
