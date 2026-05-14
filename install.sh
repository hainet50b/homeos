#!/usr/bin/env sh
# Install script for homeos
# Usage: curl -sSf https://raw.githubusercontent.com/hainet50b/homeos/main/install.sh | sh

set -e

REPO="hainet50b/homeos"
INSTALL_DIR="${HOMEOS_INSTALL_DIR:-$HOME/.local/bin}"

err() {
    echo "Error: $1" >&2
    exit 1
}

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
    Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
    Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
    *) err "Unsupported platform: $OS $ARCH" ;;
esac

command -v curl >/dev/null 2>&1 || err "curl is required but not installed"
command -v tar >/dev/null 2>&1 || err "tar is required but not installed"

URL="https://github.com/$REPO/releases/latest/download/homeos-$TARGET.tar.gz"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading homeos for $TARGET..."
curl -fsSL "$URL" -o "$TMP/homeos.tar.gz" || err "download failed: $URL"

echo "Extracting..."
tar -xzf "$TMP/homeos.tar.gz" -C "$TMP" || err "extract failed"

mkdir -p "$INSTALL_DIR"
mv "$TMP/homeos" "$INSTALL_DIR/homeos"
chmod +x "$INSTALL_DIR/homeos"

echo "Installed homeos to $INSTALL_DIR/homeos"

case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        echo ""
        "$INSTALL_DIR/homeos" --version
        ;;
    *)
        cat <<EOF

Note: $INSTALL_DIR is not in your PATH.
Add the following to your shell config (e.g., ~/.bashrc):

    export PATH="$INSTALL_DIR:\$PATH"

Then open a new terminal.
EOF
        ;;
esac
