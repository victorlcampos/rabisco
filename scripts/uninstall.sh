#!/usr/bin/env bash
# Remove o Rabisco, o início automático e as preferências.
set -euo pipefail

LABEL=io.github.victorlcampos.rabisco
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
pkill -x rabisco 2>/dev/null || true
rm -f "$HOME/Library/LaunchAgents/$LABEL.plist"
rm -rf /Applications/Rabisco.app "$HOME/Applications/Rabisco.app"
rm -rf "$HOME/Library/Application Support/Rabisco"
rm -f "$HOME/Library/Logs/Rabisco.log"
echo "Rabisco removido."
