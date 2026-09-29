#!/usr/bin/env bash
# Compila, instala o Rabisco em /Applications e o deixa rodando e abrindo junto com o Mac.
set -euo pipefail
cd "$(dirname "$0")/.."

LABEL=io.github.victorlcampos.rabisco
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DEST=/Applications
[ -w "$DEST" ] || DEST="$HOME/Applications"

./scripts/bundle.sh

# Para a instância que estiver rodando (pelo launchd ou aberta à mão).
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
pkill -x rabisco 2>/dev/null || true
sleep 1

mkdir -p "$DEST"
rm -rf "$DEST/Rabisco.app"
cp -R target/Rabisco.app "$DEST/Rabisco.app"

# O próprio app escreve o LaunchAgent apontando para onde foi instalado.
"$DEST/Rabisco.app/Contents/MacOS/rabisco" --enable-login
launchctl bootstrap "gui/$(id -u)" "$PLIST"

echo "Rabisco instalado em $DEST/Rabisco.app e rodando na barra de menus."
echo "Atalho: ⌃⇧⌘E (control + shift + command + E)"
