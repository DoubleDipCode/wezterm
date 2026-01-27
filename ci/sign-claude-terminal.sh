#!/bin/bash
# Claude Terminal macOS Code Signing Script
#
# This script signs the ClaudeTerminal.app bundle for distribution.
# It requires:
#   1. Apple Developer ID certificate installed in keychain
#   2. MACOS_TEAM_ID environment variable set to your Developer ID
#
# Usage:
#   ./ci/sign-claude-terminal.sh [APP_PATH]
#
# Example:
#   MACOS_TEAM_ID="Developer ID Application: Your Name (TEAMID)" ./ci/sign-claude-terminal.sh
#
# For CI/CD, set these environment variables:
#   MACOS_TEAM_ID - Developer ID Application certificate identity
#   MACOS_CERT - Base64-encoded .p12 certificate file
#   MACOS_CERT_PW - Base64-encoded password for the .p12 file

set -e

# Default app path
APP_PATH="${1:-assets/macos/ClaudeTerminal.app}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

echo_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

echo_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if app bundle exists
if [ ! -d "$APP_PATH" ]; then
    echo_error "App bundle not found at: $APP_PATH"
    echo "Please ensure the app bundle exists before signing."
    exit 1
fi

# Check for Developer ID
if [ -z "$MACOS_TEAM_ID" ]; then
    echo_error "MACOS_TEAM_ID environment variable not set."
    echo ""
    echo "To sign the app, you need an Apple Developer ID certificate."
    echo ""
    echo "Steps to set up code signing:"
    echo "  1. Join Apple Developer Program (\$99/year)"
    echo "  2. Create a 'Developer ID Application' certificate in Xcode or Apple Developer portal"
    echo "  3. Export the certificate to a .p12 file"
    echo "  4. Install the certificate in your keychain"
    echo "  5. Find your certificate identity with: security find-identity -v -p codesigning"
    echo "  6. Set MACOS_TEAM_ID to the certificate identity string"
    echo ""
    echo "Example:"
    echo "  export MACOS_TEAM_ID=\"Developer ID Application: Your Name (ABC123DEF4)\""
    echo "  ./ci/sign-claude-terminal.sh"
    exit 1
fi

echo_info "Starting code signing for Claude Terminal"
echo_info "App path: $APP_PATH"
echo_info "Certificate: $MACOS_TEAM_ID"

# If running in CI with certificate data, set up keychain
if [ -n "$MACOS_CERT" ]; then
    echo_info "Setting up CI keychain for code signing..."

    MACOS_PW=$(echo $MACOS_CERT_PW | base64 --decode)

    # Store current default keychain
    def_keychain=$(eval echo $(security default-keychain -d user))
    echo_info "Default keychain: $def_keychain"

    # Delete existing build keychain if present
    security delete-keychain build.keychain 2>/dev/null || true

    # Create and configure build keychain
    security create-keychain -p "$MACOS_PW" build.keychain
    security default-keychain -d user -s build.keychain
    security unlock-keychain -p "$MACOS_PW" build.keychain

    # Import certificate
    echo $MACOS_CERT | base64 --decode > /tmp/certificate.p12
    security import /tmp/certificate.p12 -k build.keychain -P "$MACOS_PW" -T /usr/bin/codesign
    rm /tmp/certificate.p12

    # Grant codesign access
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$MACOS_PW" build.keychain

    KEYCHAIN_ARG="--keychain build.keychain"
    CLEANUP_KEYCHAIN=true
else
    KEYCHAIN_ARG=""
    CLEANUP_KEYCHAIN=false
fi

# Sign the app bundle
echo_info "Signing app bundle with codesign..."
/usr/bin/codesign $KEYCHAIN_ARG \
    --force \
    --options runtime \
    --entitlements ci/claude-terminal-entitlement.plist \
    --deep \
    --sign "$MACOS_TEAM_ID" \
    "$APP_PATH"

echo_info "Code signing complete."

# Verify the signature
echo_info "Verifying signature..."
if /usr/bin/codesign --verify --verbose "$APP_PATH"; then
    echo_info "Signature verification passed!"
else
    echo_error "Signature verification failed!"
    exit 1
fi

# Check Gatekeeper assessment
echo_info "Checking Gatekeeper assessment..."
if spctl --assess --verbose "$APP_PATH" 2>&1; then
    echo_info "Gatekeeper assessment passed - app will launch without warnings."
else
    echo_warn "Gatekeeper assessment failed - app may require notarization."
    echo_warn "Run ci/notarize-claude-terminal.sh to submit for notarization."
fi

# Display detailed signature info
echo_info "Signature details:"
codesign -dvvv "$APP_PATH" 2>&1 | head -20

# Cleanup CI keychain if needed
if [ "$CLEANUP_KEYCHAIN" = true ]; then
    echo_info "Cleaning up CI keychain..."
    security default-keychain -d user -s "$def_keychain"
    security delete-keychain build.keychain || true
fi

echo ""
echo_info "Code signing completed successfully!"
echo ""
echo "Next steps:"
echo "  1. Test the app launches without Gatekeeper warnings"
echo "  2. If distributing outside App Store, run notarization:"
echo "     ./ci/notarize-claude-terminal.sh"
