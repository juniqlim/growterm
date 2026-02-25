#!/bin/bash
set -e

cargo build --release -p juniqterm-app

APP="/Applications/JuniqTerm.app"
mkdir -p "$APP/Contents/MacOS"
cp target/release/juniqterm "$APP/Contents/MacOS/juniqterm"

cat > "$APP/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>juniqterm</string>
	<key>CFBundleIdentifier</key>
	<string>com.juniqlim.juniqterm</string>
	<key>CFBundleName</key>
	<string>JuniqTerm</string>
	<key>CFBundleVersion</key>
	<string>0.1.0</string>
	<key>CFBundleShortVersionString</key>
	<string>0.1.0</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF

echo "Installed to $APP"
