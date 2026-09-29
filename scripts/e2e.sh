#!/usr/bin/env bash
# Teste de ponta a ponta: põe uma imagem no clipboard, abre o editor, rabisca pela
# própria interface, aperta ↩ e confere o clipboard pixel a pixel (src/selftest.rs).
# Atenção: substitui o conteúdo do clipboard. Uso: scripts/e2e.sh [pasta-de-saída]
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="${1:-target/e2e}"
rm -rf "$OUT"
mkdir -p "$OUT"

# A janela precisa ser desenhada: acorda o monitor, se estiver dormindo.
caffeinate -u -t 2

# Mesmo diretório de build do app: só o crate do Rabisco recompila com a feature.
cargo build --release --features selftest
cp target/release/rabisco target/rabisco-selftest

echo "1/2: clipboard sem imagem (estado vazio, esc)"
printf 'rabisco e2e' | pbcopy
RABISCO_SELFTEST="$OUT/vazio" target/rabisco-selftest
test -s "$OUT/vazio/vazio.png"

echo "2/2: print no clipboard (rabisco, ponto, ⌘Z, popup de cores, ↩)"
osascript -e "set the clipboard to (read (POSIX file \"$PWD/tests/fixtures/print.png\") as «class PNGf»)"
RABISCO_SELFTEST="$OUT/imagem" target/rabisco-selftest
test -s "$OUT/imagem/editor.png" && test -s "$OUT/imagem/popup.png" && test -s "$OUT/imagem/result.png"

echo "e2e ok — prints e resultado em $OUT/"
