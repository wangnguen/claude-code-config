#!/bin/bash
set -e

CCC_HOME="$HOME/.ccc"
REPO="wangnguen/claude-code-config"

# Detect OS and arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)  TARGET="x86_64-unknown-linux-gnu" ;;
    Darwin)
        case "$ARCH" in
            arm64) TARGET="aarch64-apple-darwin" ;;
            *)     TARGET="x86_64-apple-darwin" ;;
        esac
        ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

ASSET_NAME="ccc-${TARGET}.zip"

# A plain release download rather than api.github.com: the API allows 60
# unauthenticated requests an hour per IP, which an office sharing one
# address runs out of.
DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/$ASSET_NAME"

# Download zip
echo "Downloading $ASSET_NAME..."
TMP_ZIP=$(mktemp)
TMP_DIR=$(mktemp -d)
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_ZIP"

# Extract and copy to CccHome
unzip -o "$TMP_ZIP" -d "$TMP_DIR"
mkdir -p "$CCC_HOME"
cp -r "$TMP_DIR"/ccc-*/. "$CCC_HOME/"
chmod +x "$CCC_HOME/ccc"

# Cleanup
rm "$TMP_ZIP"
rm -rf "$TMP_DIR"

# Add to PATH
SHELL_NAME="$(basename "$SHELL")"
case "$SHELL_NAME" in
    zsh)  RC_FILE="$HOME/.zshrc" ;;
    bash) RC_FILE="$HOME/.bashrc" ;;
    *)    RC_FILE="$HOME/.profile" ;;
esac

if ! grep -q "$CCC_HOME" "$RC_FILE" 2>/dev/null; then
    echo "" >> "$RC_FILE"
    echo "# ccc - Claude Code Config" >> "$RC_FILE"
    echo "export PATH=\"\$PATH:$CCC_HOME\"" >> "$RC_FILE"
    echo "Added $CCC_HOME to PATH in $RC_FILE"
    echo "Run: source $RC_FILE  or restart your terminal."
else
    echo "$CCC_HOME is already in PATH."
fi

echo ""
echo "Done! ccc installed to $CCC_HOME"
echo "  - $CCC_HOME/ccc"
echo "  - $CCC_HOME/.claude/"
echo ""
echo "Then run: ccc key <your-api-key>"
