//! Pequenas chamadas ao AppKit que o winit não expõe.

use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSScreen};

fn app() -> Option<objc2::rc::Retained<NSApplication>> {
    MainThreadMarker::new().map(NSApplication::sharedApplication)
}

/// Traz o app para frente. Só funciona em resposta a uma ação do usuário
/// (atalho, clique no menu), que é exatamente quando chamamos.
pub fn activate() {
    if let Some(app) = app() {
        if app.respondsToSelector(sel!(activate)) {
            app.activate();
        } else {
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
    }
}

/// Esconde o app, devolvendo o foco para o app que estava na frente antes.
pub fn hide() {
    if let Some(app) = app() {
        app.hide(None);
    }
}

pub struct ScreenInfo {
    /// Área útil da tela (sem Dock e barra de menus), em pontos.
    pub visible: [f64; 2],
    /// Pixels físicos por ponto (2 em telas Retina).
    pub scale: f64,
    /// Tamanho da tela em pixels físicos.
    pub pixels: [f64; 2],
}

impl ScreenInfo {
    /// Maior lado da imagem mostrada na tela; acima disso ela é reduzida só para exibição.
    pub fn max_display_side(&self) -> u32 {
        (self.pixels[0].max(self.pixels[1]) as u32).clamp(2048, 8192)
    }
}

/// Tela que está com o foco do teclado (onde o usuário está trabalhando).
pub fn main_screen() -> ScreenInfo {
    let screen = MainThreadMarker::new().and_then(NSScreen::mainScreen);
    match screen {
        Some(s) => {
            let frame = s.frame();
            let visible = s.visibleFrame();
            let scale = s.backingScaleFactor();
            ScreenInfo {
                visible: [visible.size.width, visible.size.height],
                scale,
                pixels: [frame.size.width * scale, frame.size.height * scale],
            }
        }
        None => ScreenInfo {
            visible: [1440.0, 875.0],
            scale: 2.0,
            pixels: [2880.0, 1800.0],
        },
    }
}
