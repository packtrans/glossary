#!/usr/bin/env bash
set -euo pipefail

LANG_CODE="${LANG_CODE:-zh_cn}"
RELEASE_URL="${RELEASE_URL:-https://api.github.com/repos/packtrans/glossary-indexes/releases/latest}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${ROOT}/public/indexes/${LANG_CODE}.zip"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required to download glossary indexes." >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to parse GitHub release metadata." >&2
  exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"

echo "Fetching latest glossary index release metadata..."
release_json="$(curl -fsSL \
  -H "Accept: application/vnd.github+json" \
  -H "User-Agent: packtrans-glossary-web" \
  "${RELEASE_URL}")"

asset_url="$(printf '%s' "${release_json}" | jq -r --arg lang "${LANG_CODE}" '
  .assets[]
  | select(.name | startswith("packtrans-glossary-index-\($lang)-"))
  | .browser_download_url
' | head -n 1)"

if [[ -z "${asset_url}" || "${asset_url}" == "null" ]]; then
  echo "No ${LANG_CODE} asset found in latest glossary-indexes release." >&2
  exit 1
fi

asset_name="$(printf '%s' "${release_json}" | jq -r --arg lang "${LANG_CODE}" '
  .assets[]
  | select(.name | startswith("packtrans-glossary-index-\($lang)-"))
  | .name
' | head -n 1)"

tag_name="$(printf '%s' "${release_json}" | jq -r '.tag_name')"

echo "Downloading ${asset_name} (${tag_name})..."
curl -fsSL -o "${OUTPUT}.tmp" "${asset_url}"
mv "${OUTPUT}.tmp" "${OUTPUT}"
echo "Wrote ${OUTPUT}"
