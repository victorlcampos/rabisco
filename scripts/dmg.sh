#!/usr/bin/env bash
# Gera target/Rabisco.dmg (janela com o Rabisco, um atalho para Aplicativos e a seta)
# a partir de target/Rabisco.app. Rode ./scripts/bundle.sh antes.
set -euo pipefail
cd "$(dirname "$0")/.."

APP=target/Rabisco.app
DMG=target/Rabisco.dmg
if [ ! -d "$APP" ]; then
  echo "Não achei $APP: rode ./scripts/bundle.sh antes." >&2
  exit 1
fi

# O dmgbuild escreve o layout da janela (.DS_Store) direto, sem precisar controlar o
# Finder, o que também funciona no CI. Fica num ambiente Python próprio, dentro de target/.
VENV=target/dmg-venv
if [ ! -x "$VENV/bin/dmgbuild" ]; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install --quiet --disable-pip-version-check "dmgbuild==1.6.7"
fi

# Fundo em 1x e 2x num TIFF só, para ficar nítido em telas Retina.
tiffutil -cathidpicheck assets/dmg-background.png assets/dmg-background@2x.png \
  -out target/dmg-background.tiff >/dev/null 2>&1

rm -f "$DMG"
"$VENV/bin/dmgbuild" -s packaging/dmg-settings.py \
  -D app="$APP" -D background=target/dmg-background.tiff \
  "Rabisco" "$DMG" >/dev/null

echo "Gerado: $DMG"
