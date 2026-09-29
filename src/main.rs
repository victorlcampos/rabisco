//! Rabisco: rabisque a imagem do clipboard e devolva para o clipboard.
//!
//! Fica na barra de menus. ⌃⇧E abre o editor com a imagem que estiver no
//! clipboard; ↩ (ou o botão verde) copia o resultado de volta; esc descarta.

mod agent;
mod canvas;
mod clipboard;
mod editor;
mod icons;
mod login;
mod macos;
mod prefs;
mod reopen;
#[cfg(feature = "selftest")]
mod selftest;
mod theme;
mod window;

use std::fs::File;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

use editor::Action;
use window::EditorWindow;

#[derive(Debug)]
pub enum UserEvent {
    OpenEditor,
    Menu(String),
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    agent: Option<agent::Agent>,
    reopen: Option<Retained<reopen::ReopenHandler>>,
    editor: Option<EditorWindow>,
    prefs: prefs::Prefs,
    open_at_start: bool,
    clear_status_at: Option<Instant>,
}

impl App {
    fn open_editor(&mut self, event_loop: &ActiveEventLoop) {
        macos::activate();
        if let Some(editor) = &mut self.editor {
            // Já aberto: se ainda não rabiscou e o clipboard mudou, troca a imagem.
            if editor.can_replace() && clipboard::change_count() != editor.change_count {
                editor.reload();
            }
            editor.focus();
            return;
        }
        let change_count = clipboard::change_count();
        let loaded = clipboard::read();
        match EditorWindow::open(
            event_loop,
            loaded,
            change_count,
            self.prefs.color,
            self.prefs.size,
        ) {
            Ok(editor) => self.editor = Some(editor),
            Err(err) => eprintln!("Não consegui abrir o editor: {err}"),
        }
    }

    fn close_editor(&mut self, action: Action) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        self.prefs.color = editor.editor.color;
        self.prefs.size = editor.editor.size;
        self.prefs.save();
        editor.hide();
        #[cfg(feature = "selftest")]
        let selftest = editor.take_selftest();
        if action == Action::Save {
            match editor.export() {
                Some(Ok(())) => self.show_status("Copiado"),
                Some(Err(err)) => {
                    eprintln!("Erro ao copiar para o clipboard: {err}");
                    self.show_status("Erro ao copiar");
                }
                None => {}
            }
        }
        editor.close();
        #[cfg(feature = "selftest")]
        if let Some(test) = selftest {
            std::process::exit(test.verify());
        }
        macos::hide();
    }

    fn show_status(&mut self, text: &str) {
        if let Some(agent) = &self.agent {
            agent.set_status(Some(text));
            self.clear_status_at = Some(Instant::now() + Duration::from_millis(1800));
        }
    }

    fn toggle_login(&mut self) {
        let result = if login::is_enabled() {
            login::disable()
        } else {
            login::enable()
        };
        if let Err(err) = result {
            eprintln!("Erro ao mudar o início automático: {err}");
        }
        self.prefs.login_configured = true;
        self.prefs.save();
        if let Some(agent) = &self.agent {
            agent.set_login_checked(login::is_enabled());
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            self.agent = Some(agent::Agent::new(self.proxy.clone()));
            self.reopen = reopen::install(self.proxy.clone());
            if self.open_at_start {
                self.open_editor(event_loop);
            }
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::OpenEditor => self.open_editor(event_loop),
            UserEvent::Menu(id) => match id.as_str() {
                agent::MENU_EDIT => self.open_editor(event_loop),
                agent::MENU_LOGIN => self.toggle_login(),
                agent::MENU_QUIT => event_loop.exit(),
                _ => {}
            },
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.window.id() != id {
            return;
        }
        if let Some(action) = editor.on_event(&event) {
            self.close_editor(action);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        if let Some(editor) = &mut self.editor {
            match editor.repaint_at {
                Some(t) if t <= now => {
                    editor.repaint_at = None;
                    editor.window.request_redraw();
                }
                Some(t) => next = Some(t),
                None => {}
            }
        }
        match self.clear_status_at {
            Some(t) if t <= now => {
                self.clear_status_at = None;
                if let Some(agent) = &self.agent {
                    agent.set_status(None);
                }
            }
            Some(t) => next = Some(next.map_or(t, |n| n.min(t))),
            None => {}
        }
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

enum Lock {
    Acquired(#[allow(dead_code)] File),
    Busy,
    Unavailable,
}

/// Garante uma instância só (o atalho global não pode ser registrado duas vezes).
fn single_instance() -> Lock {
    use std::os::fd::AsRawFd;
    let dir = prefs::support_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("rabisco.lock"))
    else {
        return Lock::Unavailable;
    };
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        Lock::Acquired(file)
    } else {
        Lock::Busy
    }
}

/// Rodando de dentro de um .app instalado? Fica de fora o `cargo run` e a cópia
/// temporária que o Gatekeeper usa quando o app é aberto direto da pasta Downloads
/// ("App Translocation"): um início automático apontando para ela quebraria.
fn is_installed_bundle() -> bool {
    std::env::current_exe()
        .map(|p| {
            let p = p.to_string_lossy();
            p.contains(".app/Contents/MacOS/") && !p.contains("/AppTranslocation/")
        })
        .unwrap_or(false)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Usados pelos scripts de instalação: liga/desliga o início com o Mac e sai.
    if let Some(flag @ ("--enable-login" | "--disable-login")) = args.first().map(String::as_str) {
        let enable = flag == "--enable-login";
        let result = if enable {
            login::enable()
        } else {
            login::disable()
        };
        if let Err(err) = result {
            eprintln!("Erro: {err}");
            std::process::exit(1);
        }
        let mut prefs = prefs::Prefs::load();
        prefs.login_configured = true;
        prefs.save();
        println!(
            "Início com o Mac {}: {}",
            if enable { "ligado" } else { "desligado" },
            login::plist_path().display()
        );
        return;
    }
    let background = args.iter().any(|a| a == "--background");

    #[cfg(feature = "selftest")]
    let testing = selftest::SelfTest::enabled();
    #[cfg(not(feature = "selftest"))]
    let testing = false;

    // O teste roda ao lado de um Rabisco instalado, então não disputa a trava.
    let _lock = match if testing {
        Lock::Unavailable
    } else {
        single_instance()
    } {
        Lock::Busy => {
            eprintln!("O Rabisco já está aberto (procure o lápis na barra de menus).");
            return;
        }
        lock => lock,
    };

    let mut prefs = prefs::Prefs::load();
    if is_installed_bundle() && !testing {
        if !prefs.login_configured {
            match login::enable() {
                Ok(()) => {
                    prefs.login_configured = true;
                    prefs.save();
                }
                Err(err) => eprintln!("Não consegui ativar o início com o Mac: {err}"),
            }
        } else {
            login::refresh_if_moved();
        }
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .with_activate_ignoring_other_apps(false)
        .build()
        .expect("event loop");
    let mut app = App {
        proxy: event_loop.create_proxy(),
        agent: None,
        reopen: None,
        editor: None,
        prefs,
        open_at_start: !background,
        clear_status_at: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("Erro no event loop: {err}");
    }
}
