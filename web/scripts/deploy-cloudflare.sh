#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

export PATH="${HOME}/.vite-plus/bin:${PATH}"

vp build
vp exec wrangler deploy --temporary "$@"
