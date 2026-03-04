#!/usr/bin/env bash
set -eu

REPO="ExoMonk/evm-cloud"
BINARY_NAME="evm-cloud"

usage() {
  cat <<'EOF'
Install evm-cloud CLI from GitHub Releases.

Usage:
  install.sh [VERSION]

Examples:
  install.sh               # installs latest release
  install.sh v0.1.0        # installs specific tag

Environment variables:
  INSTALL_DIR=/custom/path/bin     # optional install location
EOF
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

resolve_latest_version() {
  curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" | sed 's|.*/tag/||'
}

detect_os() {
  os_raw=$(uname -s)
  case "$os_raw" in
    Darwin) echo "darwin" ;;
    Linux) echo "linux" ;;
    *)
      echo "Unsupported OS: $os_raw" >&2
      exit 1
      ;;
  esac
}

detect_arch() {
  arch_raw=$(uname -m)
  case "$arch_raw" in
    x86_64|amd64) echo "amd64" ;;
    arm64|aarch64) echo "arm64" ;;
    *)
      echo "Unsupported architecture: $arch_raw" >&2
      exit 1
      ;;
  esac
}

sha256_verify() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$1"
    return
  fi

  echo "Neither sha256sum nor shasum is available on this system" >&2
  exit 1
}

pick_install_dir() {
  if [ -n "${INSTALL_DIR:-}" ]; then
    echo "$INSTALL_DIR"
    return
  fi

  if [ -w "/usr/local/bin" ]; then
    echo "/usr/local/bin"
    return
  fi

  echo "$HOME/.local/bin"
}

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  VERSION=$(resolve_latest_version)
fi

OS=$(detect_os)
ARCH=$(detect_arch)
FILE="${BINARY_NAME}_${VERSION}_${OS}_${ARCH}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

ARCHIVE_PATH="$TMP_DIR/$FILE"
CHECKSUM_PATH="$TMP_DIR/checksums.txt"

echo "Installing ${BINARY_NAME} ${VERSION} (${OS}/${ARCH})"

curl -fsSL "${BASE_URL}/${FILE}" -o "$ARCHIVE_PATH"
curl -fsSL "${BASE_URL}/checksums.txt" -o "$CHECKSUM_PATH"

(
  cd "$TMP_DIR"
  grep "  ${FILE}\$" checksums.txt > checksums.match
  if [ ! -s checksums.match ]; then
    echo "No checksum entry found for ${FILE}" >&2
    exit 1
  fi
  sha256_verify checksums.match
)

tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"

INSTALL_PATH=$(pick_install_dir)
mkdir -p "$INSTALL_PATH"

if [ -w "$INSTALL_PATH" ]; then
  cp "$TMP_DIR/$BINARY_NAME" "$INSTALL_PATH/$BINARY_NAME"
else
  echo "Install directory not writable: $INSTALL_PATH" >&2
  echo "Retry with: sudo INSTALL_DIR=/usr/local/bin bash -s -- ${VERSION}" >&2
  exit 1
fi

chmod +x "$INSTALL_PATH/$BINARY_NAME"

echo "Installed to $INSTALL_PATH/$BINARY_NAME"
echo "Run: $BINARY_NAME --help"
