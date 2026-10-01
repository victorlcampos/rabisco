//! Rabisco: rabisque a imagem do clipboard e devolva para o clipboard.
//!
//! Fica na barra de menus. ⌃⇧E abre o editor com a imagem que estiver no
//! clipboard; ↩ (ou o botão verde) copia o resultado de volta; esc descarta.
//! `rabisco update` (ou *Procurar atualização*, no menu) instala a versão mais nova.

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
mod update;
mod window;

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
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
    Update(UpdateStep),
}

/// O que a atualização pedida pelo menu, que corre numa thread à parte, conta de volta.
#[derive(Debug)]
pub enum UpdateStep {
    Downloading(cerne::update::Version),
    Done(Result<update::Outcome, cerne::update::Error>),
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    agent: Option<agent::Agent>,
    reopen: Option<Retained<reopen::ReopenHandler>>,
    editor: Option<EditorWindow>,
    prefs: prefs::Prefs,
    open_at_start: bool,
    clear_status_at: Option<Instant>,
    /// Uma atualização pedida pelo menu está em andamento.
    updating: bool,
    /// A versão nova já está instalada e espera o editor fechar para reabrir o Rabisco.
    reopen_pending: bool,
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
        self.show_status_for(text, Some(Duration::from_millis(1800)));
    }

    /// Mostra `text` ao lado do ícone por `time`, ou até o próximo aviso.
    fn show_status_for(&mut self, text: &str, time: Option<Duration>) {
        if let Some(agent) = &self.agent {
            agent.set_status(Some(text));
            self.clear_status_at = time.map(|time| Instant::now() + time);
        }
    }

    /// Procura e instala a versão mais nova numa thread à parte, sem travar o menu nem o atalho.
    fn check_update(&mut self) {
        if self.updating || self.reopen_pending {
            return;
        }
        let Some(bundle) = update::installed_bundle() else {
            self.show_status_for("Só no app instalado", Some(Duration::from_secs(3)));
            return;
        };
        self.updating = true;
        self.show_status_for("Procurando…", None);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = update::update(&bundle, true, |version| {
                let _ =
                    proxy.send_event(UserEvent::Update(UpdateStep::Downloading(version.clone())));
            });
            let _ = proxy.send_event(UserEvent::Update(UpdateStep::Done(result)));
        });
    }

    fn on_update(&mut self, event_loop: &ActiveEventLoop, step: UpdateStep) {
        let shown = Some(Duration::from_secs(3));
        match step {
            UpdateStep::Downloading(version) => {
                self.show_status_for(&format!("Baixando {version}…"), None);
            }
            UpdateStep::Done(Ok(update::Outcome::Installed(_))) => {
                self.updating = false;
                self.reopen_pending = true;
                if self.editor.is_some() {
                    self.show_status_for("Atualizado: reabre ao fechar o editor", None);
                } else {
                    self.reopen(event_loop);
                }
            }
            UpdateStep::Done(Ok(_)) => {
                self.updating = false;
                self.show_status_for("Já está atualizado", shown);
            }
            UpdateStep::Done(Err(err)) => {
                self.updating = false;
                eprintln!("Erro ao atualizar: {err}");
                self.show_status_for("Erro ao atualizar", shown);
            }
        }
    }

    /// Sai, deixando quem reabra o Rabisco já na versão nova assim que este processo terminar.
    fn reopen(&mut self, event_loop: &ActiveEventLoop) {
        let reopened = update::installed_bundle()
            .ok_or_else(|| std::io::Error::other("o Rabisco.app sumiu"))
            .and_then(|bundle| update::reopen_after(std::process::id(), &bundle));
        match reopened {
            Ok(()) => event_loop.exit(),
            Err(err) => {
                eprintln!("Não consegui reabrir o Rabisco: {err}");
                self.reopen_pending = false;
                self.show_status_for("Atualizado: reabra o Rabisco", None);
            }
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
                agent::MENU_UPDATE => self.check_update(),
                agent::MENU_QUIT => event_loop.exit(),
                _ => {}
            },
            UserEvent::Update(step) => self.on_update(event_loop, step),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.window.id() != id {
            return;
        }
        if let Some(action) = editor.on_event(&event) {
            self.close_editor(action);
            if self.reopen_pending {
                self.reopen(event_loop);
            }
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

pub fn lock_path() -> PathBuf {
    prefs::support_dir().join("rabisco.lock")
}

/// Garante uma instância só (o atalho global não pode ser registrado duas vezes).
fn single_instance() -> Lock {
    use std::os::fd::AsRawFd;
    let _ = std::fs::create_dir_all(prefs::support_dir());
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path())
    else {
        return Lock::Unavailable;
    };
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        // O PID, para o `rabisco update` do terminal achar este processo e reabri-lo.
        let _ = file.set_len(0);
        let _ = write!(&file, "{}", std::process::id());
        Lock::Acquired(file)
    } else {
        Lock::Busy
    }
}

/// Encerrado pelo SIGTERM (o que o `rabisco update` do terminal manda, e o `kill`), sai como
/// quem escolheu Sair: com sucesso, e o launchd não o reabre por conta própria, na versão
/// antiga, por cima de quem vai reabrir a nova.
extern "C" fn quit_on_signal(_: libc::c_int) {
    // SAFETY: _exit é seguro dentro de um tratador de sinal.
    unsafe { libc::_exit(0) }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("rabisco {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("update") => std::process::exit(update::cli(&args[1..])),
        _ => {}
    }

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
    let handler: extern "C" fn(libc::c_int) = quit_on_signal;
    // SAFETY: o tratador só chama _exit.
    unsafe { libc::signal(libc::SIGTERM, handler as libc::sighandler_t) };

    let mut prefs = prefs::Prefs::load();
    if update::installed_bundle().is_some() && !testing {
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
        updating: false,
        reopen_pending: false,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("Erro no event loop: {err}");
    }
}
