//! Teste de ponta a ponta. Só entra no binário com `--features selftest` e roda no CI
//! pelo `scripts/e2e.sh`.
//!
//! Com `RABISCO_SELFTEST=<pasta>` e uma imagem no clipboard, o editor:
//! 1. rabisca uma onda, clica um ponto, traça uma linha e a desfaz com ⌘Z, tudo por
//!    eventos injetados na própria interface (o mesmo caminho do mouse e do teclado);
//! 2. salva prints da janela, renderizados pela GPU (não precisa de permissão de
//!    gravação de tela), em `<pasta>/editor.png` e, com o popup de cores aberto, em
//!    `<pasta>/popup.png`;
//! 3. aperta ↩; depois que o app copia o resultado, confere o clipboard pixel a pixel e
//!    salva o que encontrou em `<pasta>/result.png`.
//!
//! Sem imagem no clipboard, confere o estado vazio: salva `<pasta>/vazio.png`, aperta esc
//! e verifica que o clipboard não foi alterado.
//!
//! O processo termina com código 0 se tudo bateu, 1 se algo falhou e 3 se travou.

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, UserData, pos2};

use crate::clipboard::{self, ClipImage};
use crate::editor::Editor;

const TIMEOUT: Duration = Duration::from_secs(60);

enum Expect {
    /// O pixel tem que estar pintado com esta cor.
    Ink([u8; 4]),
    /// O pixel tem que continuar igual ao da imagem original.
    Original,
}

pub struct SelfTest {
    dir: PathBuf,
    original: Option<ClipImage>,
    frame: u32,
    shots: u32,
    popup_at: Option<u32>,
    esc_at: Option<u32>,
    checks: Vec<([f32; 2], Expect, &'static str)>,
}

fn press(input: &mut RawInput, pos: Pos2, pressed: bool) {
    input.events.push(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
}

fn drag(input: &mut RawInput, points: &[Pos2]) {
    input.events.push(Event::PointerMoved(points[0]));
    press(input, points[0], true);
    for p in &points[1..] {
        input.events.push(Event::PointerMoved(*p));
    }
    press(input, points[points.len() - 1], false);
}

fn key(input: &mut RawInput, key: Key, modifiers: Modifiers) {
    for pressed in [true, false] {
        input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}

fn fail(message: &str) -> ! {
    eprintln!("selftest: FALHOU — {message}");
    std::process::exit(1);
}

impl SelfTest {
    pub fn enabled() -> bool {
        std::env::var_os("RABISCO_SELFTEST").is_some()
    }

    pub fn from_env() -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os("RABISCO_SELFTEST")?);
        std::fs::create_dir_all(&dir).ok()?;
        // Se algo travar (ex.: a janela nunca desenhar), falha em vez de pendurar o CI.
        std::thread::spawn(|| {
            std::thread::sleep(TIMEOUT);
            eprintln!(
                "selftest: FALHOU — tempo esgotado ({}s). A janela precisa estar visível: \
                 tela desbloqueada e monitor ligado.",
                TIMEOUT.as_secs()
            );
            std::process::exit(3);
        });
        Some(Self {
            dir,
            original: clipboard::read().ok(),
            frame: 0,
            shots: 0,
            popup_at: None,
            esc_at: None,
            checks: Vec::new(),
        })
    }

    /// Injeta a ação do quadro atual. Devolve `true` quando este quadro deve ser capturado.
    pub fn before_frame(&mut self, input: &mut RawInput, editor: &mut Editor) -> bool {
        self.frame += 1;
        self.save_screenshots(&input.events);
        let f = self.frame;
        if f == 1 {
            editor.size = 1;
        }
        if self.original.is_none() {
            // Estado vazio: print, esc e pronto.
            if self.shots >= 1 && self.esc_at.is_none() {
                key(input, Key::Escape, Modifiers::NONE);
                self.esc_at = Some(f);
            }
            return f == 5;
        }
        let Some((rect, scale)) = editor.canvas_transform() else {
            if f > 5 {
                fail("o editor abriu sem imagem (o clipboard tinha uma imagem?)");
            }
            return false;
        };
        let to_image = |p: Pos2| [(p.x - rect.min.x) / scale, (p.y - rect.min.y) / scale];
        let ink = editor.color.to_srgba_unmultiplied();
        let c = rect.center();

        match f {
            3 => {
                let wave: Vec<Pos2> = (0..=40)
                    .map(|t| {
                        pos2(
                            c.x - 120.0 + 6.0 * t as f32,
                            c.y - 40.0 + 25.0 * (t as f32 / 4.0).sin(),
                        )
                    })
                    .collect();
                drag(input, &wave);
                // O traço suavizado passa exatamente pelos pontos médios entre amostras.
                self.checks.push((
                    to_image(wave[20].lerp(wave[21], 0.5)),
                    Expect::Ink(ink),
                    "meio da onda",
                ));
            }
            4 => {
                let p = pos2(c.x + 150.0, c.y - 40.0);
                drag(input, &[p]);
                self.checks
                    .push((to_image(p), Expect::Ink(ink), "ponto do clique"));
            }
            5 => {
                let line: Vec<Pos2> = (0..=10)
                    .map(|t| pos2(c.x - 100.0 + 20.0 * t as f32, c.y + 60.0))
                    .collect();
                drag(input, &line);
                self.checks.push((
                    to_image(pos2(c.x, c.y + 60.0)),
                    Expect::Original,
                    "linha desfeita com ⌘Z",
                ));
                self.checks
                    .push(([2.0, 2.0], Expect::Original, "canto sem rabisco"));
            }
            6 => key(input, Key::Z, Modifiers::COMMAND),
            9 => return true,
            _ => {}
        }
        if self.shots >= 1 && self.popup_at.is_none() {
            let chevron = editor.chevron_rect().center();
            press(input, chevron, true);
            press(input, chevron, false);
            self.popup_at = Some(f);
        }
        if self.popup_at.is_some_and(|p| f == p + 3) {
            return true;
        }
        if self.shots >= 2 && self.esc_at.is_none() {
            key(input, Key::Escape, Modifiers::NONE); // fecha o popup
            self.esc_at = Some(f);
        }
        if self.esc_at.is_some_and(|e| f == e + 2) {
            key(input, Key::Enter, Modifiers::NONE); // copia para o clipboard
        }
        false
    }

    pub fn capture_data() -> Vec<UserData> {
        vec![UserData::default()]
    }

    fn save_screenshots(&mut self, events: &[Event]) {
        for event in events {
            let Event::Screenshot { image, .. } = event else {
                continue;
            };
            let name = match (self.original.is_some(), self.shots) {
                (false, _) => "vazio.png",
                (true, 0) => "editor.png",
                (true, _) => "popup.png",
            };
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let img = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, rgba)
                .unwrap_or_else(|| fail("print com tamanho inválido"));
            if let Err(err) = img.save(self.dir.join(name)) {
                fail(&format!("não consegui salvar {name}: {err}"));
            }
            eprintln!("selftest: print salvo em {}", self.dir.join(name).display());
            self.shots += 1;
        }
    }

    /// Confere o clipboard depois do ↩ e devolve o código de saída do processo.
    pub fn verify(self) -> i32 {
        let Some(original) = self.original else {
            if self.shots < 1 {
                fail("o print do estado vazio não foi salvo");
            }
            if clipboard::read().is_ok() {
                fail("fechar com esc não deveria colocar imagem no clipboard");
            }
            eprintln!("selftest: tudo certo (estado vazio fechou com esc sem tocar no clipboard)");
            return 0;
        };
        let result = clipboard::read().unwrap_or_else(|e| fail(&e.message()));
        let _ = result.image.save(self.dir.join("result.png"));

        let mut errors = Vec::new();
        if self.shots < 2 {
            errors.push(format!(
                "esperava 2 prints da janela, saíram {}",
                self.shots
            ));
        }
        if result.image.dimensions() != original.image.dimensions() {
            errors.push(format!(
                "tamanho mudou: {:?} → {:?}",
                original.image.dimensions(),
                result.image.dimensions()
            ));
        }
        if result.dpi != original.dpi {
            errors.push(format!("DPI mudou: {:?} → {:?}", original.dpi, result.dpi));
        }
        if self.checks.len() < 4 {
            errors.push("os rabiscos não chegaram a ser feitos".into());
        }
        for (p, expect, what) in &self.checks {
            let (x, y) = (p[0].floor() as u32, p[1].floor() as u32);
            if x >= result.image.width() || y >= result.image.height() {
                errors.push(format!("{what}: ({x}, {y}) fora da imagem"));
                continue;
            }
            let got = result.image.get_pixel(x, y).0;
            let want = match expect {
                Expect::Ink(c) => *c,
                Expect::Original => original.image.get_pixel(x, y).0,
            };
            let ok = got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 2);
            eprintln!(
                "selftest: {} {what} em ({x}, {y}): esperado {want:?}, veio {got:?}",
                if ok { "ok" } else { "ERRO" }
            );
            if !ok {
                errors.push(format!(
                    "{what} em ({x}, {y}): esperado {want:?}, veio {got:?}"
                ));
            }
        }

        if errors.is_empty() {
            eprintln!(
                "selftest: tudo certo ({} verificações)",
                self.checks.len() + 3
            );
            0
        } else {
            for e in &errors {
                eprintln!("selftest: FALHOU — {e}");
            }
            1
        }
    }
}
