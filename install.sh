#!/usr/bin/env bash
set -euo pipefail

REPO="ThomasPasquali/sbatchman"
VERSION="${1:-latest}"
INSTALL_DIR_DEFAULT="$HOME/.local/bin"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
  Linux-x86_64)  TARGET="x86_64-unknown-linux-musl" ;;
  Linux-aarch64) TARGET="aarch64-unknown-linux-musl" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
  *) echo "Unsupported platform: $OS-$ARCH"; exit 1 ;;
esac

echo "Install directory [default: $INSTALL_DIR_DEFAULT]: "
read -r INSTALL_DIR
INSTALL_DIR="${INSTALL_DIR:-$INSTALL_DIR_DEFAULT}"
mkdir -p "$INSTALL_DIR"

# Download URLs
if [ "$VERSION" = "latest" ]; then
  API_URL="https://api.github.com/repos/$REPO/releases/latest"
else
  API_URL="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
fi

BIN_URL=$(curl -fsSL "$API_URL" | grep "browser_download_url" | grep "$TARGET\"" | cut -d '"' -f 4)
SHA_URL=$(curl -fsSL "$API_URL" | grep "browser_download_url" | grep "$TARGET\.sha256" | cut -d '"' -f 4)

if [ -z "$BIN_URL" ] || [ -z "$SHA_URL" ]; then
  echo "Error: could not find binary or checksum for $TARGET"
  exit 1
fi

TMP_BIN="/tmp/sbatchman-${TARGET}"
TMP_SHA="${TMP_BIN}.sha256"

echo "Downloading binary..."
curl -fsSL "$BIN_URL" -o "$TMP_BIN"
chmod +x "$TMP_BIN"

echo "Downloading checksum..."
curl -fsSL "$SHA_URL" -o "$TMP_SHA"

# Verify checksum
echo "Verifying checksum..."
cd /tmp
if command -v sha256sum >/dev/null; then
  sha256sum -c "$(basename "$TMP_SHA")"
elif command -v shasum >/dev/null; then
  shasum -a 256 -c "$(basename "$TMP_SHA")"
else
  echo "No sha256 tool found, skipping verification."
fi
cd - >/dev/null

# Install
mv "$TMP_BIN" "$INSTALL_DIR/sbatchman"
rm -f "$TMP_SHA"

# Add to PATH if needed
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  SHELL_RC="$HOME/.bashrc"
  [[ "$SHELL" == *"zsh"* ]] && SHELL_RC="$HOME/.zshrc"
  echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
  export PATH="$PATH:$INSTALL_DIR"
fi

echo "Verifying installation..."
if ! command -v sbatchman >/dev/null; then
  echo "Installation failed."
  exit 1
fi

echo "✅ sbatchman installed successfully!"
