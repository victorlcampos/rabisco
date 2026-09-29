//! Cores e medidas da interface, inspiradas no editor de fotos do WhatsApp Web.

use egui::Color32;

/// Cores da paleta principal (a mesma ordem da barra do WhatsApp).
pub const PALETTE: [Color32; 8] = [
    Color32::from_rgb(0x3b, 0x3b, 0x3b),
    Color32::from_rgb(0x9a, 0x9c, 0xa3),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x5a, 0xc8, 0xfa),
    Color32::from_rgb(0x7e, 0xd9, 0x57),
    Color32::from_rgb(0xb1, 0x7c, 0xee),
    Color32::from_rgb(0xe9, 0x96, 0x3c),
    Color32::from_rgb(0xea, 0x5a, 0x4f),
];

/// Cores extras, abertas pela setinha ao lado da paleta.
pub const MORE_COLORS: [Color32; 24] = [
    Color32::from_rgb(0x00, 0x00, 0x00),
    Color32::from_rgb(0x3b, 0x4a, 0x54),
    Color32::from_rgb(0x66, 0x77, 0x81),
    Color32::from_rgb(0x86, 0x96, 0xa0),
    Color32::from_rgb(0xae, 0xba, 0xc1),
    Color32::from_rgb(0xd1, 0xd7, 0xdb),
    Color32::from_rgb(0xe9, 0xed, 0xef),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xff, 0x2d, 0x55),
    Color32::from_rgb(0xff, 0x3b, 0x30),
    Color32::from_rgb(0xff, 0x95, 0x00),
    Color32::from_rgb(0xff, 0xcc, 0x00),
    Color32::from_rgb(0x34, 0xc7, 0x59),
    Color32::from_rgb(0x00, 0xc7, 0xbe),
    Color32::from_rgb(0x00, 0x7a, 0xff),
    Color32::from_rgb(0xaf, 0x52, 0xde),
    Color32::from_rgb(0x8e, 0x0e, 0x1c),
    Color32::from_rgb(0xa0, 0x52, 0x2d),
    Color32::from_rgb(0xc7, 0x7c, 0x02),
    Color32::from_rgb(0x6b, 0x8e, 0x23),
    Color32::from_rgb(0x0b, 0x6e, 0x4f),
    Color32::from_rgb(0x0a, 0x4f, 0x8c),
    Color32::from_rgb(0x3d, 0x2c, 0x8d),
    Color32::from_rgb(0xd6, 0x33, 0x84),
];

/// Cor inicial do lápis (vermelho, a mais útil para marcar prints).
pub const DEFAULT_COLOR: Color32 = PALETTE[7];

/// Espessura do traço, em pontos de tela, para cada tamanho.
pub const STROKE_WIDTHS: [f32; 4] = [3.0, 6.0, 11.0, 18.0];

/// Diâmetro das bolinhas que representam cada tamanho na barra.
pub const SIZE_DOTS: [f32; 4] = [8.0, 13.0, 19.0, 25.0];

pub struct Theme {
    pub bg: Color32,
    pub separator: Color32,
    pub icon: Color32,
    pub icon_disabled: Color32,
    pub hover: Color32,
    pub tool_fill: Color32,
    pub ring: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub size_on: Color32,
    pub size_off: Color32,
    pub swatch_border: Color32,
    pub hint: Color32,
    pub text: Color32,
    pub shadow: Color32,
    pub popup_bg: Color32,
    pub toast_bg: Color32,
    pub toast_text: Color32,
}

impl Theme {
    pub fn new(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    fn light() -> Self {
        Self {
            bg: Color32::from_rgb(0xff, 0xff, 0xff),
            separator: Color32::from_rgb(0xe9, 0xed, 0xef),
            icon: Color32::from_rgb(0x54, 0x65, 0x6f),
            icon_disabled: Color32::from_rgb(0xc5, 0xcd, 0xd2),
            hover: Color32::from_rgb(0xf0, 0xf2, 0xf5),
            tool_fill: Color32::from_rgb(0xf5, 0xf6, 0xf2),
            ring: Color32::from_rgb(0x3c, 0xb3, 0x6b),
            accent: Color32::from_rgb(0x1d, 0xaa, 0x61),
            accent_hover: Color32::from_rgb(0x18, 0x98, 0x56),
            on_accent: Color32::WHITE,
            size_on: Color32::from_rgb(0x11, 0x1b, 0x21),
            size_off: Color32::from_rgb(0xc4, 0xc9, 0xce),
            swatch_border: Color32::from_rgb(0x54, 0x65, 0x6f),
            hint: Color32::from_rgb(0x86, 0x96, 0xa0),
            text: Color32::from_rgb(0x11, 0x1b, 0x21),
            shadow: Color32::from_black_alpha(34),
            popup_bg: Color32::WHITE,
            toast_bg: Color32::from_rgba_unmultiplied(0x11, 0x1b, 0x21, 230),
            toast_text: Color32::WHITE,
        }
    }

    fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(0x11, 0x1b, 0x21),
            separator: Color32::from_rgb(0x22, 0x2e, 0x35),
            icon: Color32::from_rgb(0xae, 0xba, 0xc1),
            icon_disabled: Color32::from_rgb(0x3b, 0x4a, 0x54),
            hover: Color32::from_rgb(0x22, 0x2e, 0x35),
            tool_fill: Color32::from_rgb(0x2a, 0x39, 0x42),
            ring: Color32::from_rgb(0x21, 0xc0, 0x63),
            accent: Color32::from_rgb(0x21, 0xc0, 0x63),
            accent_hover: Color32::from_rgb(0x1d, 0xaa, 0x58),
            on_accent: Color32::from_rgb(0x0b, 0x14, 0x1a),
            size_on: Color32::from_rgb(0xe9, 0xed, 0xef),
            size_off: Color32::from_rgb(0x3b, 0x4a, 0x54),
            swatch_border: Color32::from_rgb(0x86, 0x96, 0xa0),
            hint: Color32::from_rgb(0x86, 0x96, 0xa0),
            text: Color32::from_rgb(0xe9, 0xed, 0xef),
            shadow: Color32::from_black_alpha(90),
            popup_bg: Color32::from_rgb(0x23, 0x32, 0x3b),
            toast_bg: Color32::from_rgba_unmultiplied(0xe9, 0xed, 0xef, 235),
            toast_text: Color32::from_rgb(0x11, 0x1b, 0x21),
        }
    }

    /// Uma bolinha de cor precisa de borda quando some contra o fundo.
    pub fn swatch_needs_border(&self, color: Color32) -> bool {
        let lum = |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
        (lum(color) - lum(self.bg)).abs() < 40.0
    }
}
