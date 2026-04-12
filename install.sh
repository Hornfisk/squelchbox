#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

case "$(uname -s)" in
  Linux)
    VST3_DIR="$HOME/.vst3/squelchbox.vst3/Contents/x86_64-linux"
    CLAP_DIR="$HOME/.clap"
    PLUGIN="libsquelchbox.so"
    VST3_NAME="squelchbox.so"
    CLAP_NAME="squelchbox.clap"
    ;;
  Darwin)
    VST3_DIR="$HOME/Library/Audio/Plug-Ins/VST3/squelchbox.vst3/Contents/MacOS"
    CLAP_DIR="$HOME/Library/Audio/Plug-Ins/CLAP"
    PLUGIN="libsquelchbox.dylib"
    VST3_NAME="squelchbox"
    CLAP_NAME="squelchbox.clap"
    ;;
  *)
    echo "Unsupported OS. Use install.bat on Windows."
    exit 1
    ;;
esac

STANDALONE="squelchbox-standalone"

if [ ! -f "$SCRIPT_DIR/$PLUGIN" ]; then
  echo "Error: $PLUGIN not found in $SCRIPT_DIR"
  echo "Make sure install.sh is in the same folder as the built binaries."
  exit 1
fi

echo "Installing SquelchBox plugins..."

# VST3
mkdir -p "$VST3_DIR"
cp "$SCRIPT_DIR/$PLUGIN" "$VST3_DIR/$VST3_NAME"
echo "  VST3 -> $VST3_DIR/$VST3_NAME"

# CLAP
mkdir -p "$CLAP_DIR"
cp "$SCRIPT_DIR/$PLUGIN" "$CLAP_DIR/$CLAP_NAME"
echo "  CLAP -> $CLAP_DIR/$CLAP_NAME"

# Standalone
if [ -f "$SCRIPT_DIR/$STANDALONE" ]; then
  BIN_DIR="$HOME/.local/bin"
  mkdir -p "$BIN_DIR"
  cp "$SCRIPT_DIR/$STANDALONE" "$BIN_DIR/squelchbox"
  chmod +x "$BIN_DIR/squelchbox"
  echo "  Standalone -> $BIN_DIR/squelchbox"
fi

echo ""
echo "Done! Rescan plugins in your DAW to find SquelchBox."
