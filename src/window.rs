//! A janela do editor: winit para a janela e eventos, egui para a interface e
//! egui-wgpu (Metal) para desenhar. Ela é criada a cada atalho e destruída ao fechar,
//! então o app parado na barra de menus quase não usa memória.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, ViewportId};
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::canvas::Document;
use crate::clipboard::{self, ClipImage, ReadError};
use crate::editor::{self, Action, Editor};
use crate::macos::{self, ScreenInfo};
use crate::theme::Theme;

const MIN_W: f64 = 640.0;
const MIN_H: f64 = 480.0;

pub struct EditorWindow {
    pub window: Arc<Window>,
    ctx: egui::Context,
    state: egui_winit::State,
    painter: egui_wgpu::winit::Painter,
    pub editor: Editor,
    /// `changeCount` do clipboard quando a imagem foi lida.
    pub change_count: isize,
    /// Quando o egui pediu para ser redesenhado (tooltips, avisos que somem...).
    pub repaint_at: Option<Instant>,
    screen: ScreenInfo,
    shown: bool,
    #[cfg(feature = "selftest")]
    selftest: Option<crate::selftest::SelfTest>,
}

fn document_from(
    loaded: Result<ClipImage, ReadError>,
    screen: &ScreenInfo,
) -> (Option<Document>, String) {
    match loaded {
        Ok(clip) => (
            Some(Document::new(
                clip.image,
                clip.dpi,
                screen.max_display_side(),
            )),
            String::new(),
        ),
        Err(err) => (None, err.message()),
    }
}

/// Tamanho inicial: a imagem em 100% (como aparecia na tela) mais as barras,
/// limitado a ~92% da tela.
fn window_size(doc: Option<&Document>, screen: &ScreenInfo) -> LogicalSize<f64> {
    let max_w = (screen.visible[0] * 0.92).max(MIN_W);
    let max_h = (screen.visible[1] * 0.92 - 28.0).max(MIN_H);
    let Some(doc) = doc else {
        return LogicalSize::new(MIN_W, MIN_H);
    };
    let size = doc.size();
    let scale = Editor::preferred_scale(size, screen.scale as f32) as f64;
    let w = size[0] as f64 * scale + 2.0 * editor::SIDE_MARGIN as f64;
    let h = size[1] as f64 * scale + (editor::TOP_H + editor::BOTTOM_H + 20.0) as f64;
    LogicalSize::new(w.clamp(MIN_W, max_w), h.clamp(MIN_H, max_h))
}

/// Usa a fonte do sistema (SF) quando disponível.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts
            .font_data
            .insert("sf".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "sf".into());
    }
    ctx.set_fonts(fonts);
}

impl EditorWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        loaded: Result<ClipImage, ReadError>,
        change_count: isize,
        color: Color32,
        size: usize,
    ) -> Result<Self, String> {
        let screen = macos::main_screen();
        let (doc, message) = document_from(loaded, &screen);
        let attributes = Window::default_attributes()
            .with_title("Rabisco")
            .with_inner_size(window_size(doc.as_ref(), &screen))
            .with_min_inner_size(LogicalSize::new(MIN_W, MIN_H))
            .with_visible(false);
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| e.to_string())?,
        );

        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            ctx.clone(),
            egui_wgpu::WgpuConfiguration::default(),
            false,
            egui_wgpu::RendererOptions::default(),
        ));
        pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
            .map_err(|e| e.to_string())?;
        let state = egui_winit::State::new(
            ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );
        window.request_redraw();

        Ok(Self {
            window,
            ctx,
            state,
            painter,
            editor: Editor::new(doc, message, color, size),
            change_count,
            repaint_at: None,
            screen,
            shown: false,
            #[cfg(feature = "selftest")]
            selftest: crate::selftest::SelfTest::from_env(),
        })
    }

    pub fn focus(&self) {
        macos::activate();
        self.window.focus_window();
    }

    /// Ainda não há rabiscos, então dá para trocar a imagem sem perder nada.
    pub fn can_replace(&self) -> bool {
        !self.editor.doc.as_ref().is_some_and(|d| d.is_edited())
    }

    /// Lê o clipboard de novo e ajusta a janela à nova imagem.
    pub fn reload(&mut self) {
        self.change_count = clipboard::change_count();
        let (doc, message) = document_from(clipboard::read(), &self.screen);
        if doc.is_some() {
            let _ = self
                .window
                .request_inner_size(window_size(doc.as_ref(), &self.screen));
        }
        self.editor.doc = doc;
        self.editor.message = message;
        self.window.request_redraw();
    }

    pub fn on_event(&mut self, event: &WindowEvent) -> Option<Action> {
        match event {
            WindowEvent::CloseRequested => return Some(Action::Cancel),
            WindowEvent::RedrawRequested => return self.redraw(),
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter.on_window_resized(ViewportId::ROOT, w, h);
                }
                self.window.request_redraw();
            }
            WindowEvent::Focused(false) => self.editor.end_stroke(),
            _ => {}
        }
        if self.state.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
        None
    }

    fn redraw(&mut self) -> Option<Action> {
        #[cfg_attr(not(feature = "selftest"), allow(unused_mut))]
        let mut input = self.state.take_egui_input(&self.window);
        #[cfg_attr(not(feature = "selftest"), allow(unused_mut))]
        let mut capture = Vec::new();
        #[cfg(feature = "selftest")]
        if let Some(test) = &mut self.selftest {
            self.painter.handle_screenshots(&mut input.events);
            if test.before_frame(&mut input, &mut self.editor) {
                capture = crate::selftest::SelfTest::capture_data();
            }
        }
        let ctx = self.ctx.clone();
        let editor = &mut self.editor;
        let mut action = None;
        let output = ctx.run_ui(input, |ui| {
            if let Some(a) = editor.ui(ui) {
                action = Some(a);
            }
        });
        self.state
            .handle_platform_output(&self.window, output.platform_output);
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        let mut textures = output.textures_delta;
        let clear = Theme::new(self.editor.dark).bg.to_normalized_gamma_f32();
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            output.pixels_per_point,
            clear,
            &primitives,
            &mut textures,
            capture,
            &self.window,
        );

        let delay = output
            .viewport_output
            .get(&ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        self.repaint_at = None;
        #[cfg(feature = "selftest")]
        if self.selftest.is_some() {
            self.window.request_redraw(); // o teste avança um passo por quadro
        }
        if delay.is_zero() {
            self.window.request_redraw();
        } else {
            self.repaint_at = Instant::now().checked_add(delay);
        }

        // A janela nasce invisível e só aparece depois do primeiro quadro, sem piscar.
        if !self.shown {
            self.shown = true;
            self.window.set_visible(true);
            self.focus();
        }
        if std::mem::take(&mut self.editor.reload_requested) {
            self.reload();
        }
        action
    }

    /// Gera a imagem final e a coloca no clipboard.
    pub fn export(&mut self) -> Option<Result<(), String>> {
        let doc = self.editor.doc.as_mut()?;
        doc.end();
        let image = doc.render();
        Some(clipboard::write(&image, doc.dpi))
    }

    #[cfg(feature = "selftest")]
    pub fn take_selftest(&mut self) -> Option<crate::selftest::SelfTest> {
        self.selftest.take()
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn close(mut self) {
        self.hide();
        let _ = pollster::block_on(self.painter.set_window(ViewportId::ROOT, None));
        self.painter.destroy();
    }
}
