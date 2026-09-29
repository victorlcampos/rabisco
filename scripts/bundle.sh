#!/usr/bin/env bash
# Gera target/Rabisco.app: binário release, ícone e assinatura ad-hoc.
# Com UNIVERSAL=1, o binário roda em Apple Silicon e Intel (usado nas releases).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP=target/Rabisco.app
ICONSET=target/AppIcon.iconset

if [ "${UNIVERSAL:-0}" = 1 ]; then
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  BIN=target/rabisco-universal
  lipo -create -output "$BIN" \
    target/aarch64-apple-darwin/release/rabisco \
    target/x86_64-apple-darwin/release/rabisco
else
  cargo build --release
  BIN=target/release/rabisco
fi

rm -rf "$APP" "$ICONSET"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET"

# O ícone é desenhado pelo próprio binário, em todos os tamanhos que o macOS usa.
for size in 16 32 128 256 512; do
  "$BIN" --render-icon "$ICONSET/icon_${size}x${size}.png" "$size"
  "$BIN" --render-icon "$ICONSET/icon_${size}x${size}@2x.png" "$((size * 2))"
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

sed "s/__VERSION__/$VERSION/g" packaging/Info.plist > "$APP/Contents/Info.plist"
cp "$BIN" "$APP/Contents/MacOS/rabisco"
codesign --force --sign - "$APP" >/dev/null

echo "Gerado: $APP (versão $VERSION)"
