#!/usr/bin/env bash
# Download only packtrans-glossary from the latest GitHub release into project bin/.
set -euo pipefail

REPO="${PACKTRANS_GLOSSARY_REPO:-packtrans/glossary}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BIN_DIR="${BIN_DIR:-${PROJECT_ROOT}/bin}"
VERSION="${VERSION:-}"

CURL_API_OPTS=(--retry 3 --retry-all-errors --retry-connrefused --connect-timeout 10 --max-time 30)
CURL_DOWNLOAD_OPTS=(--retry 3 --retry-all-errors --retry-connrefused --connect-timeout 10 --max-time 120)

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "${os}/${arch}" in
    Linux/x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux/aarch64)
      echo "error: no Linux aarch64 release asset; build from source with cargo" >&2
      return 1
      ;;
    Darwin/arm64) echo "aarch64-apple-darwin" ;;
    Darwin/x86_64)
      echo "error: no Intel macOS release asset; use Apple Silicon build or cargo" >&2
      return 1
      ;;
    MINGW*/x86_64 | MSYS*/x86_64 | CYGWIN*/x86_64) echo "x86_64-pc-windows-msvc" ;;
    *)
      echo "error: unsupported platform ${os}/${arch}" >&2
      return 1
      ;;
  esac
}

fetch_latest_tag() {
  curl -fsSL "${CURL_API_OPTS[@]}" \
    "https://api.github.com/repos/${REPO}/releases/latest" |
    sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p' | head -1
}

_install_tmpdir=""

cleanup() {
  if [[ -n "${_install_tmpdir}" && -d "${_install_tmpdir}" ]]; then
    rm -rf "${_install_tmpdir}"
  fi
}

main() {
  local target tag archive_name url archive_path binary_name dest

  trap cleanup EXIT
  target="$(detect_target)"
  if [[ -z "${VERSION}" ]]; then
    tag="$(fetch_latest_tag)"
    if [[ -z "${tag}" ]]; then
      echo "error: failed to resolve latest release tag for ${REPO}" >&2
      return 1
    fi
  else
    tag="${VERSION}"
    [[ "${tag}" == v* ]] || tag="v${tag}"
  fi

  mkdir -p "${BIN_DIR}"

  if [[ "${target}" == *windows* ]]; then
    archive_name="packtrans-glossary-${tag}-${target}.zip"
    binary_name="packtrans-glossary.exe"
    url="https://github.com/${REPO}/releases/download/${tag}/${archive_name}"
    _install_tmpdir="$(mktemp -d)"
    archive_path="${_install_tmpdir}/${archive_name}"
    curl -fsSL "${CURL_DOWNLOAD_OPTS[@]}" -o "${archive_path}" "${url}"
    unzip -q "${archive_path}" -d "${_install_tmpdir}/extract"
    dest="${BIN_DIR}/${binary_name}"
    cp "${_install_tmpdir}/extract/packtrans-glossary-${tag}-${target}/${binary_name}" "${dest}"
  else
    archive_name="packtrans-glossary-${tag}-${target}.tar.gz"
    binary_name="packtrans-glossary"
    url="https://github.com/${REPO}/releases/download/${tag}/${archive_name}"
    _install_tmpdir="$(mktemp -d)"
    archive_path="${_install_tmpdir}/${archive_name}"
    curl -fsSL "${CURL_DOWNLOAD_OPTS[@]}" -o "${archive_path}" "${url}"
    tar -xzf "${archive_path}" -C "${_install_tmpdir}"
    dest="${BIN_DIR}/${binary_name}"
    cp "${_install_tmpdir}/packtrans-glossary-${tag}-${target}/${binary_name}" "${dest}"
    chmod +x "${dest}"
  fi

  trap - EXIT
  cleanup
  _install_tmpdir=""

  echo "Installed ${dest} (${tag}, ${target})"
  "${dest}" --help >/dev/null
}

main "$@"
