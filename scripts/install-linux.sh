#!/usr/bin/env bash
# Сборка Reader и установка для текущего пользователя (без root):
# программа — в ~/.local/bin, иконки и ярлык — в меню приложений.
# Повторный запуск обновляет установленную версию.
#
#   ./scripts/install-linux.sh            — собрать и установить
#   ./scripts/install-linux.sh --skip-build — установить уже собранное
#   ./scripts/install-linux.sh --uninstall  — удалить
set -euo pipefail

APP_ID="com.adaman.reader"
BIN_NAME="reader"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$HOME/.local/bin"
DESKTOP_FILE="$PREFIX/applications/$APP_ID.desktop"
ICON_SIZES=(32 48 64 128 256 512)

uninstall() {
  rm -f "$BIN_DIR/$BIN_NAME" "$DESKTOP_FILE"
  for s in "${ICON_SIZES[@]}"; do
    rm -f "$PREFIX/icons/hicolor/${s}x${s}/apps/$APP_ID.png"
  done
  refresh_caches
  echo "Reader удалён. Данные библиотеки (~/.config/$APP_ID) не тронуты."
}

refresh_caches() {
  command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/applications" || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$PREFIX/icons/hicolor" || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  uninstall
  exit 0
fi

cd "$ROOT"

if [[ "${1:-}" != "--skip-build" ]]; then
  echo "→ Сборка (первый раз займёт несколько минут)…"
  [[ -d node_modules ]] || npm ci
  npm run tauri build -- --no-bundle
fi

BIN="$ROOT/src-tauri/target/release/$BIN_NAME"
[[ -x "$BIN" ]] || { echo "Не найден собранный файл: $BIN" >&2; exit 1; }

echo "→ Установка в $BIN_DIR"
mkdir -p "$BIN_DIR"
install -m 755 "$BIN" "$BIN_DIR/$BIN_NAME"

echo "→ Иконки"
SRC_ICON="$ROOT/src-tauri/icons/icon.png"
for s in "${ICON_SIZES[@]}"; do
  dir="$PREFIX/icons/hicolor/${s}x${s}/apps"
  mkdir -p "$dir"
  if command -v magick >/dev/null; then
    magick "$SRC_ICON" -resize "${s}x${s}" "$dir/$APP_ID.png"
  else
    case "$s" in
      32) cp "$ROOT/src-tauri/icons/32x32.png" "$dir/$APP_ID.png" ;;
      128) cp "$ROOT/src-tauri/icons/128x128.png" "$dir/$APP_ID.png" ;;
      256) cp "$ROOT/src-tauri/icons/128x128@2x.png" "$dir/$APP_ID.png" ;;
      *) cp "$SRC_ICON" "$dir/$APP_ID.png" ;;
    esac
  fi
done

echo "→ Ярлык в меню приложений"
mkdir -p "$(dirname "$DESKTOP_FILE")"
cat >"$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Reader
GenericName=Читалка книг
Comment=Читалка и личная библиотека: PDF, EPUB, FB2
Exec=$BIN_DIR/$BIN_NAME
Icon=$APP_ID
Terminal=false
Categories=Office;Viewer;
Keywords=книги;читалка;pdf;epub;fb2;ebook;reader;
StartupWMClass=$BIN_NAME
EOF

refresh_caches
echo "Готово: Reader появится в меню приложений (иногда нужно перезайти в сеанс)."
