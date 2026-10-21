#!/usr/bin/env bash
# Builds the desktop version for the current OS into dist/<os>.
set -euo pipefail
cd "$(dirname "$0")/.."

case "$(uname -s)" in
    Linux*)  OS=linux ;;
    Darwin*) exec scripts/bundle-macos.sh ;;
    MINGW*|MSYS*|CYGWIN*) OS=windows ;;
    *) echo "Unsupported OS"; exit 1 ;;
esac

cargo build --release
rm -rf "dist/$OS"
mkdir -p "dist/$OS"
if [ "$OS" = windows ]; then
    cp target/release/rusty-scape.exe "dist/$OS/"
else
    cp target/release/rusty-scape "dist/$OS/"
    chmod +x "dist/$OS/rusty-scape"
fi
echo "Desktop build ready in dist/$OS"
