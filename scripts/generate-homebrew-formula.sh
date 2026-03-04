#!/usr/bin/env bash
set -eu

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  echo "Usage: $0 <version-tag> <checksums-file> [output-path]" >&2
  echo "Example: $0 v0.1.0 release-assets/checksums.txt release-assets/evm-cloud.rb" >&2
  exit 1
fi

VERSION_TAG="$1"
CHECKSUMS_FILE="$2"
OUT_PATH="${3:-scripts/homebrew/evm-cloud.rb}"

if [ ! -f "$CHECKSUMS_FILE" ]; then
  echo "Checksums file not found: $CHECKSUMS_FILE" >&2
  exit 1
fi

VERSION_NO_V="${VERSION_TAG#v}"
RELEASE_BASE_URL="https://github.com/ExoMonk/evm-cloud/releases/download/${VERSION_TAG}"

asset_name() {
  local os="$1"
  local arch="$2"
  echo "evm-cloud_${VERSION_TAG}_${os}_${arch}.tar.gz"
}

asset_sha() {
  local file_name
  file_name="$1"
  local sha
  sha=$(awk -v file="$file_name" '$2 == file { print $1 }' "$CHECKSUMS_FILE")
  if [ -z "$sha" ]; then
    echo "Missing checksum for ${file_name} in ${CHECKSUMS_FILE}" >&2
    exit 1
  fi
  echo "$sha"
}

DARWIN_ARM64_ASSET=$(asset_name darwin arm64)
DARWIN_AMD64_ASSET=$(asset_name darwin amd64)
LINUX_ARM64_ASSET=$(asset_name linux arm64)
LINUX_AMD64_ASSET=$(asset_name linux amd64)

DARWIN_ARM64_SHA=$(asset_sha "$DARWIN_ARM64_ASSET")
DARWIN_AMD64_SHA=$(asset_sha "$DARWIN_AMD64_ASSET")
LINUX_ARM64_SHA=$(asset_sha "$LINUX_ARM64_ASSET")
LINUX_AMD64_SHA=$(asset_sha "$LINUX_AMD64_ASSET")

mkdir -p "$(dirname "$OUT_PATH")"

cat > "$OUT_PATH" <<EOF
class EvmCloud < Formula
  desc "CLI for deploying EVM blockchain data infrastructure"
  homepage "https://github.com/ExoMonk/evm-cloud"
  version "${VERSION_NO_V}"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "${RELEASE_BASE_URL}/${DARWIN_ARM64_ASSET}"
      sha256 "${DARWIN_ARM64_SHA}"
    else
      url "${RELEASE_BASE_URL}/${DARWIN_AMD64_ASSET}"
      sha256 "${DARWIN_AMD64_SHA}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "${RELEASE_BASE_URL}/${LINUX_ARM64_ASSET}"
      sha256 "${LINUX_ARM64_SHA}"
    else
      url "${RELEASE_BASE_URL}/${LINUX_AMD64_ASSET}"
      sha256 "${LINUX_AMD64_SHA}"
    end
  end

  def install
    bin.install "evm-cloud"
  end

  test do
    assert_match "Deploy EVM blockchain data infrastructure on AWS", shell_output("#{bin}/evm-cloud --help")
  end
end
EOF

echo "Generated formula at $OUT_PATH"
