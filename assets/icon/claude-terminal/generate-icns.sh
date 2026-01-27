#!/bin/bash
# Script to generate claude-terminal.icns from a 1024x1024 PNG
#
# Usage: ./generate-icns.sh icon-1024.png
#
# The input PNG should be 1024x1024 pixels. This script will create
# an iconset with all required sizes and convert it to .icns format.

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <input-1024x1024.png>"
    echo "Creates claude-terminal.icns from a 1024x1024 PNG source"
    exit 1
fi

INPUT="$1"
ICONSET="claude-terminal.iconset"
OUTPUT="claude-terminal.icns"

# Verify input exists
if [ ! -f "$INPUT" ]; then
    echo "Error: Input file '$INPUT' not found"
    exit 1
fi

# Check input dimensions
WIDTH=$(sips -g pixelWidth "$INPUT" | tail -1 | awk '{print $2}')
HEIGHT=$(sips -g pixelHeight "$INPUT" | tail -1 | awk '{print $2}')

if [ "$WIDTH" != "1024" ] || [ "$HEIGHT" != "1024" ]; then
    echo "Warning: Input is ${WIDTH}x${HEIGHT}, expected 1024x1024"
    echo "Continuing anyway..."
fi

# Create iconset directory
rm -rf "$ICONSET"
mkdir "$ICONSET"

# Generate all required sizes for macOS iconset
# @1x sizes
sips -z 16 16     "$INPUT" --out "$ICONSET/icon_16x16.png"
sips -z 32 32     "$INPUT" --out "$ICONSET/icon_16x16@2x.png"
sips -z 32 32     "$INPUT" --out "$ICONSET/icon_32x32.png"
sips -z 64 64     "$INPUT" --out "$ICONSET/icon_32x32@2x.png"
sips -z 128 128   "$INPUT" --out "$ICONSET/icon_128x128.png"
sips -z 256 256   "$INPUT" --out "$ICONSET/icon_128x128@2x.png"
sips -z 256 256   "$INPUT" --out "$ICONSET/icon_256x256.png"
sips -z 512 512   "$INPUT" --out "$ICONSET/icon_256x256@2x.png"
sips -z 512 512   "$INPUT" --out "$ICONSET/icon_512x512.png"
sips -z 1024 1024 "$INPUT" --out "$ICONSET/icon_512x512@2x.png"

# Convert iconset to icns
iconutil -c icns "$ICONSET" -o "$OUTPUT"

# Clean up
rm -rf "$ICONSET"

echo "Created $OUTPUT successfully"
echo ""
echo "To install, copy to the app bundle:"
echo "  cp $OUTPUT ../../macos/ClaudeTerminal.app/Contents/Resources/"
