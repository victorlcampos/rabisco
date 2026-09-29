//! Preferências simples (cor e espessura do lápis), num arquivo texto em
//! ~/Library/Application Support/Rabisco.

use std::path::PathBuf;

use egui::Color32;

use crate::theme::{DEFAULT_COLOR, STROKE_WIDTHS};

pub struct Prefs {
    pub color: Color32,
    pub size: usize,
    /// Já configuramos o início com o Mac alguma vez (para não religar se o usuário desligou).
    pub login_configured: bool,
}

pub fn support_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library/Application Support/Rabisco")
}

fn file() -> PathBuf {
    support_dir().join("prefs.txt")
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

impl Prefs {
    pub fn load() -> Self {
        let mut prefs = Prefs {
            color: DEFAULT_COLOR,
            size: 1,
            login_configured: false,
        };
        let Ok(text) = std::fs::read_to_string(file()) else {
            return prefs;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "color" => prefs.color = parse_hex(value).unwrap_or(prefs.color),
                "size" => {
                    prefs.size = value
                        .trim()
                        .parse::<usize>()
                        .unwrap_or(1)
                        .min(STROKE_WIDTHS.len() - 1)
                }
                "login_configured" => prefs.login_configured = value.trim() == "1",
                _ => {}
            }
        }
        prefs
    }

    pub fn save(&self) {
        let [r, g, b, _] = self.color.to_srgba_unmultiplied();
        let text = format!(
            "color=#{r:02x}{g:02x}{b:02x}\nsize={}\nlogin_configured={}\n",
            self.size,
            u8::from(self.login_configured)
        );
        let _ = std::fs::create_dir_all(support_dir());
        let _ = std::fs::write(file(), text);
    }
}
