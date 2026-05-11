#!/usr/bin/env sh
set -eu

REPO="distillation-labs/contextro"
BINARY="contextro"

# Resolve latest version from GitHub if not set
VERSION="${CONTEXTRO_VERSION:-}"
if [ -z "$VERSION" ]; then
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | sed 's/.*"tag_name": *"\(.*\)".*/\1/')
fi

# Detect OS
OS=$(uname -s)
case "$OS" in
  Darwin) OS="apple-darwin" ;;
  Linux)  OS="unknown-linux-gnu" ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# Detect arch
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)          ARCH="x86_64" ;;
  arm64 | aarch64) ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

TARGET="${ARCH}-${OS}"
ARCHIVE="${BINARY}-${TARGET}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

# Determine install dir — prefer ~/.local/bin (no sudo required on any platform)
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "Installing contextro ${VERSION} (${TARGET}) → ${INSTALL_DIR}/${BINARY}"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Download archive and checksums
curl -fsSL "${BASE_URL}/${ARCHIVE}" -o "${TMP}/${ARCHIVE}"
curl -fsSL "${BASE_URL}/SHA256SUMS" -o "${TMP}/SHA256SUMS"

# Verify checksum — sha256sum on Linux, shasum on macOS
cd "$TMP"
if command -v sha256sum > /dev/null 2>&1; then
  grep "${ARCHIVE}" SHA256SUMS | sha256sum -c -
else
  grep "${ARCHIVE}" SHA256SUMS | shasum -a 256 -c -
fi
cd - > /dev/null

# Extract and install
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"
install -m 755 "${TMP}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

echo ""
echo "✓ contextro installed to ${INSTALL_DIR}/${BINARY}"

# Warn if install dir is not in PATH
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "  Add ${INSTALL_DIR} to your PATH:"
    echo "    export PATH=\"\$PATH:${INSTALL_DIR}\""
    echo ""
    echo "  To make it permanent, add the above line to ~/.zshrc or ~/.bashrc"
    ;;
esac

echo ""
echo "Quick start:"
echo "  claude mcp add contextro -- contextro"
echo "  # or add to your MCP config: { \"command\": \"contextro\" }"
