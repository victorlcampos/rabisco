//! Presença na barra de menus: ícone, menu e o atalho global ⌃⇧E.
//!
//! O atalho usa `RegisterEventHotKey` (Carbon), que não exige permissão de
//! Acessibilidade. E de editar; ⌃ e ⇧ já estão na mão de quem acabou de tirar um
//! print para o clipboard (⇧⌘4 segurando ⌃): é só soltar o ⌘ e apertar o E.
//! O macOS não usa ⌃⇧E, e os apps raramente usam atalhos só com control.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::menu::accelerator::{self, Accelerator};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};
use winit::event_loop::EventLoopProxy;

use crate::{UserEvent, icons, login};

pub const SHORTCUT: &str = "⌃⇧E";
pub const MENU_EDIT: &str = "edit";
pub const MENU_LOGIN: &str = "login";
pub const MENU_UPDATE: &str = "update";
pub const MENU_QUIT: &str = "quit";

pub struct Agent {
    tray: TrayIcon,
    login_item: CheckMenuItem,
    _hotkeys: GlobalHotKeyManager,
}

impl Agent {
    /// Precisa rodar na thread principal, com o event loop já iniciado.
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let hotkeys = GlobalHotKeyManager::new().expect("gerenciador de atalhos");
        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyE);
        let hotkey_ok = match hotkeys.register(hotkey) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("Não consegui registrar o atalho {SHORTCUT}: {err}");
                false
            }
        };

        let edit_label = if hotkey_ok {
            "Editar imagem do clipboard"
        } else {
            "Editar imagem do clipboard (atalho em uso por outro app)"
        };
        let shortcut = Accelerator::new(
            accelerator::Modifiers::CONTROL | accelerator::Modifiers::SHIFT,
            accelerator::Code::KeyE,
        );
        let edit = MenuItem::with_id(MENU_EDIT, edit_label, true, Some(shortcut));
        let login_item = CheckMenuItem::with_id(
            MENU_LOGIN,
            "Abrir ao iniciar o Mac",
            true,
            login::is_enabled(),
            None,
        );
        let update = MenuItem::with_id(MENU_UPDATE, "Procurar atualização", true, None);
        let quit = MenuItem::with_id(MENU_QUIT, "Sair do Rabisco", true, None);
        let menu = Menu::new();
        let separator = PredefinedMenuItem::separator();
        let separator2 = PredefinedMenuItem::separator();
        if let Err(err) =
            menu.append_items(&[&edit, &separator, &login_item, &update, &separator2, &quit])
        {
            eprintln!("Erro montando o menu: {err}");
        }

        let tray = TrayIconBuilder::new()
            .with_icon(icons::tray_icon())
            .with_icon_as_template(true)
            .with_tooltip(format!(
                "Rabisco {} — {SHORTCUT} edita a imagem do clipboard",
                env!("CARGO_PKG_VERSION")
            ))
            .with_menu(Box::new(menu))
            .build()
            .expect("ícone na barra de menus");

        let hotkey_proxy = proxy.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                let _ = hotkey_proxy.send_event(UserEvent::OpenEditor);
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(event.id.0));
        }));

        Self {
            tray,
            login_item,
            _hotkeys: hotkeys,
        }
    }

    pub fn set_login_checked(&self, checked: bool) {
        self.login_item.set_checked(checked);
    }

    /// Texto curto ao lado do ícone (ex.: "Copiado"), ou `None` para limpar.
    pub fn set_status(&self, text: Option<&str>) {
        self.tray.set_title(text);
    }
}
