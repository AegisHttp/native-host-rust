#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

# Get the directory where the script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/dist"

echo "======================================"
echo " Packaging Aegis Http Ecosystem"
echo " (Cross-platform builds via 'cross')"
echo "======================================"

# Ensure 'cross' is installed
if ! command -v cross &> /dev/null; then
    echo "❌ HATA: 'cross' kurulu değil."
    echo "Lütfen şu komutla kurun: cargo install cross"
    exit 1
fi

echo "[1/4] Creating output directory at $OUTPUT_DIR..."
mkdir -p "$OUTPUT_DIR"

cd "$SCRIPT_DIR"

# Targets to build (Target_Triple | Artifact_Name | Binary_Name | Nfpm_Arch)
TARGETS=(
    "x86_64-unknown-linux-gnu|aegis-http-native-host-linux-amd64|aegis-host|amd64"
    "aarch64-unknown-linux-gnu|aegis-http-native-host-linux-arm64|aegis-host|arm64"
    "x86_64-pc-windows-gnu|aegis-http-native-host-windows-amd64|aegis-host.exe|"
    "x86_64-apple-darwin|aegis-http-native-host-macos-amd64|aegis-host|"
    "aarch64-apple-darwin|aegis-http-native-host-macos-arm64|aegis-host|"
)

echo "[2/4] Building cross-platform Native Hosts..."
for ENTRY in "${TARGETS[@]}"; do
    IFS="|" read -r TARGET ARTIFACT_NAME BINARY_NAME NFPM_ARCH <<< "$ENTRY"
    echo " 🚀 Building target: $TARGET"
    
    # Skip apple-darwin targets on non-macOS hosts
    if [[ "$TARGET" == *"apple-darwin"* ]] && [[ "$(uname)" != "Darwin" ]]; then
        echo " ⚠️  Skipping macOS target '$TARGET' locally (macOS SDK required). GitHub Actions will handle this."
        continue
    fi
    
    # Run cross build (Fallback to cargo for Windows due to cross-rs bug)
    if [[ "$TARGET" == *"windows"* ]]; then
        echo " 🛠️  Using native cargo for windows target..."
        cargo build --release --target "$TARGET"
    else
        cross build --release --target "$TARGET"
    fi
    
    # Staging
    STAGING_DIR="$OUTPUT_DIR/$ARTIFACT_NAME"
    rm -rf "$STAGING_DIR"
    mkdir -p "$STAGING_DIR/assets"
    
    # Copy files
    cp "target/$TARGET/release/$BINARY_NAME" "$STAGING_DIR/"
    cp install.sh "$STAGING_DIR/"
    
    # Inject CHROME_EXTENSION_ID into the zip's install.sh if provided
    if [ -n "$CHROME_EXTENSION_ID" ]; then
        sed -i "s/<PUT_YOUR_CHROME_EXTENSION_ID_HERE>/$CHROME_EXTENSION_ID/g" "$STAGING_DIR/install.sh"
    fi
    
    cp -r assets/* "$STAGING_DIR/assets/"
    cp README*.md "$STAGING_DIR/"
    cp LICENSE "$STAGING_DIR/" 2>/dev/null || true
    
    # Zip
    cd "$STAGING_DIR"
    zip -r "$OUTPUT_DIR/${ARTIFACT_NAME}.zip" . -q
    cd "$SCRIPT_DIR"
    rm -rf "$STAGING_DIR"
    echo " ✅ Packaged $ARTIFACT_NAME.zip"

    # NFPM Linux Packaging (Deb, Rpm, Archlinux)
    if [ -n "$NFPM_ARCH" ]; then
        if command -v nfpm &> /dev/null; then
            echo " 📦 Generating system packages (.deb, .rpm, .apk) for $NFPM_ARCH via nfpm..."
            export ARCH="$NFPM_ARCH"
            cp "target/$TARGET/release/$BINARY_NAME" "target/aegis-host"
            
            # Inject CHROME_EXTENSION_ID if provided
            if [ -n "$CHROME_EXTENSION_ID" ]; then
                echo " 🔧 Injecting CHROME_EXTENSION_ID ($CHROME_EXTENSION_ID) into chrome.json for packaging..."
                cp packaging/chrome.json packaging/chrome.json.bak
                sed -i "s/<PUT_YOUR_CHROME_EXTENSION_ID_HERE>/$CHROME_EXTENSION_ID/g" packaging/chrome.json
            fi

            nfpm pkg --config packaging/nfpm.yaml --target "$OUTPUT_DIR/" --packager deb
            nfpm pkg --config packaging/nfpm.yaml --target "$OUTPUT_DIR/" --packager rpm
            nfpm pkg --config packaging/nfpm.yaml --target "$OUTPUT_DIR/" --packager archlinux
            
            # Restore original chrome.json
            if [ -n "$CHROME_EXTENSION_ID" ] && [ -f "packaging/chrome.json.bak" ]; then
                mv packaging/chrome.json.bak packaging/chrome.json
            fi
            
            rm -f "target/aegis-host"
        else
            echo " ⚠️  'nfpm' command not found, skipping OS package generation for Linux."
        fi
    fi
done

echo "======================================"
echo "✅ Done! Packages successfully created:"
ls -lh "$OUTPUT_DIR"/*.zip
echo "======================================"
