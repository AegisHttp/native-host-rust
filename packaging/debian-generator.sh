#!/bin/bash
set -e

VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

# Clean 'v' prefix if present
VERSION="${VERSION#v}"

echo "[+] Generating debian/ directory for version $VERSION"

mkdir -p debian/source

# Write debian/compat
echo "10" > debian/compat

# Write debian/source/format
echo "3.0 (native)" > debian/source/format

# Write debian/control
cat <<EOF > debian/control
Source: aegis-host
Section: utils
Priority: optional
Maintainer: Aegis HTTP Team <me@canus.dev>
Build-Depends: debhelper (>= 10), cargo, rustc
Standards-Version: 4.6.0

Package: aegis-host
Architecture: any
Depends: \${shlibs:Depends}, \${misc:Depends}, gnupg
Description: Aegis Http HTTP GPG Daemon
 This daemon implements the Native Messaging API to bridge GPG capabilities
 with browser extensions securely.
EOF

# Write debian/rules
cat <<EOF > debian/rules
#!/usr/bin/make -f
%:
	dh \$@

override_dh_auto_build:
	cargo build --release

override_dh_auto_install:
	install -Dm755 target/release/aegis-host debian/aegis-host/usr/bin/aegis-host
EOF
chmod +x debian/rules

# Write debian/changelog
DATE=$(date -R)
cat <<EOF > debian/changelog
aegis-host ($VERSION) unstable; urgency=medium

  * Release version $VERSION.

 -- Aegis HTTP Team <me@canus.dev>  $DATE
EOF

echo "[+] Debian directory generation complete."
