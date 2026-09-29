# Layout da janela do Rabisco.dmg, para o dmgbuild (https://dmgbuild.readthedocs.io).
# Uso: scripts/dmg.sh, que passa -D app=... e -D background=...
import os.path

application = defines.get("app", "target/Rabisco.app")  # noqa: F821 (definido pelo dmgbuild)
appname = os.path.basename(application)

format = "ULFO"  # compressão lzfse (macOS 10.11+)
filesystem = "HFS+"
size = None
files = [application]
symlinks = {"Applications": "/Applications"}

# Ícone do volume montado: o mesmo do app.
icon = os.path.join(application, "Contents", "Resources", "AppIcon.icns")

# Janela: fundo com a seta (assets/dmg-background.svg) e os dois ícones nas posições
# em que o desenho espera.
background = defines.get("background", "target/dmg-background.tiff")  # noqa: F821
window_rect = ((200, 140), (660, 400))
icon_locations = {appname: (180, 180), "Applications": (480, 180)}
default_view = "icon-view"
icon_size = 128
text_size = 13
label_pos = "bottom"
show_icon_preview = False
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
