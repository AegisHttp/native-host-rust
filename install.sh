#!/bin/bash
set -e

echo "[+] Building Rust Native Host - HTTP Aegis Http..."
cargo build --release

TARGET_BIN="$(pwd)/target/release/aegis-host"

if [ ! -f "$TARGET_BIN" ]; then
    echo "[-] Build failed. Binary not found."
    exit 1
fi

echo "[+] Binary built successfully at: $TARGET_BIN"

# --- CHROME ---
CHROME_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
mkdir -p "$CHROME_DIR"
EXT_ID=${CHROME_EXTENSION_ID:-"lappbcambkogfmigiphapgjcglafcfnd"}
cat <<EOF > "$CHROME_DIR/com.aegis.http.gpg.json"
{
  "name": "com.aegis.http.gpg",
  "description": "Aegis Http HTTP GPG Daemon",
  "path": "$TARGET_BIN",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://$EXT_ID/"
  ]
}
EOF
echo "[+] Wrote Chrome Manifest to $CHROME_DIR/com.aegis.http.gpg.json"
if [ "$EXT_ID" = "lappbcambkogfmigiphapgjcglafcfnd" ]; then
    echo "    -> WARNING: You must edit this file to insert your exact Chrome Extension ID."
fi

# --- FIREFOX ---
MOZILLA_DIR="$HOME/.mozilla/native-messaging-hosts"
mkdir -p "$MOZILLA_DIR"
cat <<EOF > "$MOZILLA_DIR/com.aegis.http.gpg.json"
{
  "name": "com.aegis.http.gpg",
  "description": "Aegis Http HTTP GPG Daemon",
  "path": "$TARGET_BIN",
  "type": "stdio",
  "allowed_extensions": [
    "gpg-login@aegis.http"
  ]
}
EOF
echo "[+] Wrote Firefox Manifest to $MOZILLA_DIR/com.aegis.http.gpg.json"

echo "--------------------------------------------------------"
echo "[+] Installation complete. Restart your browser extension."
