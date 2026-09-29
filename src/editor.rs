//! Interface do editor: barra do lápis, área de desenho e paleta, no estilo do
//! editor de fotos do WhatsApp Web.

use std::time::Duration;

use egui::{
    Align2, Color32, CursorIcon, Event, FontId, Id, Key, Modifiers, Painter, PointerButton, Pos2,
    Rect, Sense, Shadow, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::canvas::Document;
use crate::icons;
use crate::theme::{MORE_COLORS, PALETTE, SIZE_DOTS, STROKE_WIDTHS, Theme};

pub const TOP_H: f32 = 64.0;
pub const BOTTOM_H: f32 = 88.0;
pub const SIDE_MARGIN: f32 = 24.0;
/// Imagens pequenas são ampliadas até ocupar pelo menos isto (em pontos).
pub const MIN_DISPLAY: f32 = 360.0;

const SWATCH: f32 = 26.0;
const SWATCH_PITCH: f32 = 38.0;
const CHEVRON_W: f32 = 30.0;
const SIZE_GAP: f32 = 16.0;
const GROUP_GAP: f32 = 36.0;
const SEND: f32 = 56.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Save,
    Cancel,
}

pub struct Editor {
    pub doc: Option<Document>,
    /// Mensagem do estado vazio (sem imagem no clipboard).
    pub message: String,
    pub color: Color32,
    pub size: usize,
    /// Pedido do botão "Tentar de novo" (ler o clipboard outra vez).
    pub reload_requested: bool,
    pub dark: bool,
    more_colors: bool,
    chevron_rect: Rect,
    discard_armed_until: Option<f64>,
    checker: Option<TextureHandle>,
    /// Onde a imagem foi desenhada no último quadro e com que escala (pontos por pixel).
    last_canvas: Option<(Rect, f32)>,
}

impl Editor {
    pub fn new(doc: Option<Document>, message: String, color: Color32, size: usize) -> Self {
        Self {
            doc,
            message,
            color,
            size: size.min(STROKE_WIDTHS.len() - 1),
            reload_requested: false,
            dark: false,
            more_colors: false,
            chevron_rect: Rect::NOTHING,
            discard_armed_until: None,
            checker: None,
            last_canvas: None,
        }
    }

    #[cfg_attr(not(feature = "selftest"), allow(dead_code))]
    pub fn canvas_transform(&self) -> Option<(Rect, f32)> {
        self.last_canvas
    }

    #[cfg_attr(not(feature = "selftest"), allow(dead_code))]
    pub fn chevron_rect(&self) -> Rect {
        self.chevron_rect
    }

    /// Escala (pontos por pixel da imagem) que a imagem teria sem limite de espaço.
    pub fn preferred_scale(image_size: [f32; 2], pixels_per_point: f32) -> f32 {
        let native = 1.0 / pixels_per_point;
        native.max(MIN_DISPLAY / image_size[0].max(image_size[1]))
    }

    /// Termina um traço em andamento (por exemplo, quando a janela perde o foco).
    pub fn end_stroke(&mut self) {
        if let Some(doc) = &mut self.doc {
            doc.end();
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Option<Action> {
        self.dark = ui.visuals().dark_mode;
        let th = Theme::new(self.dark);
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, th.bg);

        let top = Rect::from_min_size(full.min, vec2(full.width(), TOP_H));
        let bottom = Rect::from_min_max(pos2(full.min.x, full.max.y - BOTTOM_H), full.max);
        let area = Rect::from_min_max(
            pos2(full.min.x + SIDE_MARGIN, top.max.y + 16.0),
            pos2(full.max.x - SIDE_MARGIN, bottom.min.y - 4.0),
        );

        self.last_canvas = None;
        let mut action = self.shortcuts(ui);
        self.top_bar(ui, top, &th);
        if self.doc.is_some() {
            self.canvas(ui, area, &th);
            if let Some(a) = self.bottom_bar(ui, bottom, &th) {
                action = Some(a);
            }
            self.more_colors_popup(ui, &th);
        } else {
            self.empty_state(ui, area, &th);
        }
        self.toast(ui, area, &th);

        if action == Some(Action::Save) {
            self.end_stroke();
        }
        action
    }

    fn shortcuts(&mut self, ui: &mut Ui) -> Option<Action> {
        let (redo, undo, save, cancel) = ui.input_mut(|i| {
            let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || i.consume_key(Modifiers::COMMAND, Key::Y);
            let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
            let save = i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::COMMAND, Key::Enter)
                || i.consume_key(Modifiers::COMMAND, Key::S)
                || i.consume_key(Modifiers::COMMAND, Key::C);
            let cancel = i.consume_key(Modifiers::NONE, Key::Escape)
                || i.consume_key(Modifiers::COMMAND, Key::W);
            (redo, undo, save, cancel)
        });

        if let Some(doc) = &mut self.doc {
            if redo {
                doc.redo();
            } else if undo {
                doc.undo();
            }
        }

        if cancel {
            if self.more_colors {
                self.more_colors = false;
                return None;
            }
            let now = ui.input(|i| i.time);
            let edited = self.doc.as_ref().is_some_and(|d| d.is_edited());
            if !edited || self.discard_armed_until.is_some_and(|t| now < t) {
                return Some(Action::Cancel);
            }
            // Com rabiscos na tela, o primeiro Esc só avisa.
            self.discard_armed_until = Some(now + 2.5);
            ui.ctx().request_repaint_after(Duration::from_millis(2600));
        }

        if save {
            if self.doc.is_some() {
                return Some(Action::Save);
            }
            self.reload_requested = true;
        }
        None
    }

    fn top_bar(&mut self, ui: &mut Ui, rect: Rect, th: &Theme) {
        let painter = ui.painter().clone();
        painter.hline(
            rect.x_range(),
            rect.max.y - 0.5,
            Stroke::new(1.0, th.separator),
        );
        let cy = rect.center().y;

        // O lápis, única ferramenta por enquanto, sempre selecionado.
        let tool = Rect::from_center_size(pos2(rect.center().x, cy), vec2(46.0, 46.0));
        ui.interact(tool, Id::new("tool-pencil"), Sense::hover())
            .on_hover_text("Lápis");
        painter.circle_filled(tool.center(), 23.0, th.tool_fill);
        painter.circle_stroke(tool.center(), 22.0, Stroke::new(2.0, th.ring));
        icons::pencil(&painter, tool.center(), 22.0, th.icon);

        let Some(doc) = &mut self.doc else { return };
        let redo_rect =
            Rect::from_center_size(pos2(rect.max.x - SIDE_MARGIN - 18.0, cy), vec2(38.0, 38.0));
        let undo_rect = redo_rect.translate(vec2(-46.0, 0.0));
        let (can_undo, can_redo) = (doc.can_undo(), doc.can_redo());
        if icon_button(
            ui,
            undo_rect,
            "undo",
            can_undo,
            "Desfazer (⌘Z)",
            th,
            |p, c, col| icons::undo(p, c, 21.0, col, false),
        ) {
            doc.undo();
        }
        if icon_button(
            ui,
            redo_rect,
            "redo",
            can_redo,
            "Refazer (⇧⌘Z)",
            th,
            |p, c, col| icons::undo(p, c, 21.0, col, true),
        ) {
            doc.redo();
        }
    }

    fn canvas(&mut self, ui: &mut Ui, area: Rect, th: &Theme) {
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let popup = self.more_colors.then(|| self.popup_rect(ui.max_rect()));
        let Some(doc) = self.doc.as_mut() else { return };

        let [iw, ih] = doc.size();
        let fit = (area.width() / iw).min(area.height() / ih);
        let scale = Self::preferred_scale([iw, ih], ppp).min(fit).max(1e-4);
        let size = vec2(iw * scale, ih * scale);
        let min = area.center() - size / 2.0;
        // Alinhado aos pixels físicos, para o print aparecer nítido em 100%.
        let min = pos2((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp);
        let rect = Rect::from_min_size(min, size);
        self.last_canvas = Some((rect, scale));

        let response = ui.interact(rect, Id::new("canvas"), Sense::click_and_drag());
        let to_image = |p: Pos2| [(p.x - rect.min.x) / scale, (p.y - rect.min.y) / scale];
        let color = self.color.to_srgba_unmultiplied();
        let width = STROKE_WIDTHS[self.size] / scale;

        let (events, focused, primary_down) =
            ui.input(|i| (i.events.clone(), i.focused, i.pointer.primary_down()));
        for event in &events {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                } => {
                    if popup.is_some_and(|p| p.contains(*pos)) || !rect.contains(*pos) {
                        continue;
                    }
                    if self.more_colors {
                        // Clique fora do popup de cores só o fecha.
                        self.more_colors = false;
                        continue;
                    }
                    doc.begin(to_image(*pos), color, width);
                }
                Event::PointerMoved(pos) if doc.is_drawing() => doc.extend(to_image(*pos)),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } if doc.is_drawing() => {
                    doc.extend(to_image(*pos));
                    doc.end();
                }
                _ => {}
            }
        }
        if doc.is_drawing() && (!focused || !primary_down) {
            doc.end();
        }

        let painter = ui.painter();
        painter.add(
            Shadow {
                offset: [0, 2],
                blur: 18,
                spread: 0,
                color: th.shadow,
            }
            .as_shape(rect, 0.0),
        );
        if doc.has_alpha {
            let checker = self.checker.get_or_insert_with(|| checker_texture(&ctx));
            let uv =
                Rect::from_min_max(Pos2::ZERO, pos2(rect.width() / 16.0, rect.height() / 16.0));
            painter.image(checker.id(), rect, uv, Color32::WHITE);
        }
        let texture = doc.texture(&ctx).id();
        painter.image(
            texture,
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        let clipped = painter.with_clip_rect(rect);
        if let Some((a, b, c, w)) = doc.tail() {
            let to_screen = |p: [f32; 2]| rect.min + vec2(p[0], p[1]) * scale;
            let col = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
            let w = w * scale;
            clipped.line_segment([to_screen(a), to_screen(b)], Stroke::new(w, col));
            clipped.circle_filled(to_screen(a), w / 2.0, col);
            clipped.circle_filled(to_screen(b), w / 2.0, col);
        }

        if response.hovered() || doc.is_drawing() {
            ctx.set_cursor_icon(CursorIcon::Crosshair);
        }
        if !doc.is_drawing()
            && popup.is_none()
            && let Some(p) = response.hover_pos()
        {
            let r = STROKE_WIDTHS[self.size] / 2.0;
            clipped.circle_stroke(p, r + 1.0, Stroke::new(1.0, Color32::from_black_alpha(110)));
            clipped.circle_stroke(p, r, Stroke::new(1.5, self.color));
        }
    }

    /// Largura total da paleta + tamanhos, para centralizar a barra de baixo.
    fn controls_width() -> f32 {
        let sizes: f32 = SIZE_DOTS.iter().sum::<f32>() + SIZE_GAP * (SIZE_DOTS.len() - 1) as f32;
        PALETTE.len() as f32 * SWATCH_PITCH + CHEVRON_W + GROUP_GAP + sizes
    }

    fn bottom_bar(&mut self, ui: &mut Ui, rect: Rect, th: &Theme) -> Option<Action> {
        let painter = ui.painter().clone();
        let cy = rect.center().y;
        let send_center = pos2(rect.max.x - SIDE_MARGIN - SEND / 2.0, cy);
        let total = Self::controls_width();
        let right_limit = send_center.x - SEND / 2.0 - 16.0;
        let mut x = rect.center().x - total / 2.0;
        if x + total > right_limit {
            x = (right_limit - total).max(rect.min.x + 12.0);
        }

        for (i, &col) in PALETTE.iter().enumerate() {
            let c = pos2(x + SWATCH_PITCH * (i as f32 + 0.5), cy);
            let hit = Rect::from_center_size(c, vec2(SWATCH_PITCH, SWATCH_PITCH));
            let resp = ui.interact(hit, Id::new(("swatch", i)), Sense::click());
            if resp.clicked() {
                self.color = col;
                self.more_colors = false;
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let grow = if resp.hovered() { 1.5 } else { 0.0 };
            swatch(&painter, c, SWATCH / 2.0 + grow, col, self.color == col, th);
        }

        let chevron = Rect::from_center_size(
            pos2(
                x + SWATCH_PITCH * PALETTE.len() as f32 + CHEVRON_W / 2.0,
                cy,
            ),
            vec2(CHEVRON_W, 40.0),
        );
        self.chevron_rect = chevron;
        let resp = ui
            .interact(chevron, Id::new("more-colors"), Sense::click())
            .on_hover_text("Mais cores");
        if resp.hovered() {
            painter.rect_filled(chevron, 8.0, th.hover);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        icons::chevron(&painter, chevron.center(), 20.0, th.icon, !self.more_colors);
        if !PALETTE.contains(&self.color) {
            painter.circle_filled(
                pos2(chevron.center().x, chevron.max.y - 3.0),
                3.5,
                self.color,
            );
        }
        if resp.clicked() {
            self.more_colors = !self.more_colors;
        }

        let mut sx = chevron.max.x + GROUP_GAP;
        for (i, &d) in SIZE_DOTS.iter().enumerate() {
            let c = pos2(sx + d / 2.0, cy);
            let hit = Rect::from_center_size(c, vec2(d + SIZE_GAP, 44.0));
            let resp = ui
                .interact(hit, Id::new(("size", i)), Sense::click())
                .on_hover_text(["Fino", "Médio", "Grosso", "Bem grosso"][i]);
            if resp.clicked() {
                self.size = i;
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let col = if self.size == i {
                th.size_on
            } else if resp.hovered() {
                th.size_off.lerp_to_gamma(th.size_on, 0.35)
            } else {
                th.size_off
            };
            painter.circle_filled(c, d / 2.0, col);
            sx += d + SIZE_GAP;
        }

        let send = Rect::from_center_size(send_center, vec2(SEND, SEND));
        let resp = ui
            .interact(send, Id::new("save"), Sense::click())
            .on_hover_text("Copiar para o clipboard (↩)");
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        painter.circle_filled(
            send_center,
            SEND / 2.0,
            if resp.hovered() {
                th.accent_hover
            } else {
                th.accent
            },
        );
        icons::check(&painter, send_center, 26.0, th.on_accent);
        resp.clicked().then_some(Action::Save)
    }

    fn popup_rect(&self, screen: Rect) -> Rect {
        let (cols, rows) = (8.0, (MORE_COLORS.len() / 8) as f32);
        let (sw, gap, pad) = (24.0, 10.0, 14.0);
        let size = vec2(
            cols * sw + (cols - 1.0) * gap + 2.0 * pad,
            rows * sw + (rows - 1.0) * gap + 2.0 * pad,
        );
        let x = (self.chevron_rect.center().x - size.x / 2.0).clamp(
            screen.min.x + 8.0,
            (screen.max.x - size.x - 8.0).max(screen.min.x + 8.0),
        );
        Rect::from_min_size(pos2(x, self.chevron_rect.min.y - 10.0 - size.y), size)
    }

    fn more_colors_popup(&mut self, ui: &mut Ui, th: &Theme) {
        if !self.more_colors {
            return;
        }
        let rect = self.popup_rect(ui.max_rect());
        let ctx = ui.ctx().clone();
        let mut picked = None;
        egui::Area::new(Id::new("more-colors-popup"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .fixed_pos(rect.min)
            .show(&ctx, |ui| {
                let (area, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                let painter = ui.painter();
                painter.add(
                    Shadow {
                        offset: [0, 4],
                        blur: 20,
                        spread: 0,
                        color: th.shadow,
                    }
                    .as_shape(area, 14.0),
                );
                painter.rect_filled(area, 14.0, th.popup_bg);
                let (sw, gap, pad) = (24.0, 10.0, 14.0);
                for (i, &col) in MORE_COLORS.iter().enumerate() {
                    let (row, column) = ((i / 8) as f32, (i % 8) as f32);
                    let c = area.min
                        + vec2(
                            pad + sw / 2.0 + column * (sw + gap),
                            pad + sw / 2.0 + row * (sw + gap),
                        );
                    let hit = Rect::from_center_size(c, vec2(sw + gap, sw + gap));
                    let resp = ui.interact(hit, Id::new(("more-color", i)), Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let grow = if resp.hovered() { 1.5 } else { 0.0 };
                    swatch(ui.painter(), c, sw / 2.0 + grow, col, self.color == col, th);
                    if resp.clicked() {
                        picked = Some(col);
                    }
                }
            });
        if let Some(col) = picked {
            self.color = col;
            self.more_colors = false;
        }
    }

    fn empty_state(&mut self, ui: &mut Ui, area: Rect, th: &Theme) {
        let painter = ui.painter().clone();
        let c = area.center();
        icons::image(&painter, c - vec2(0.0, 78.0), 52.0, th.hint);
        painter.text(
            c - vec2(0.0, 22.0),
            Align2::CENTER_CENTER,
            &self.message,
            FontId::proportional(20.0),
            th.text,
        );
        painter.text(
            c + vec2(0.0, 10.0),
            Align2::CENTER_CENTER,
            "Copie uma imagem (um print com ⌃⇧⌘4, por exemplo)",
            FontId::proportional(14.0),
            th.hint,
        );
        painter.text(
            c + vec2(0.0, 30.0),
            Align2::CENTER_CENTER,
            "e aperte ⌃⇧⌘E para rabiscar em cima dela.",
            FontId::proportional(14.0),
            th.hint,
        );
        let button = Rect::from_center_size(c + vec2(0.0, 82.0), vec2(168.0, 40.0));
        let resp = ui.interact(button, Id::new("retry"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        painter.rect_filled(
            button,
            20.0,
            if resp.hovered() {
                th.accent_hover
            } else {
                th.accent
            },
        );
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            "Tentar de novo",
            FontId::proportional(15.0),
            th.on_accent,
        );
        if resp.clicked() {
            self.reload_requested = true;
        }
    }

    fn toast(&mut self, ui: &mut Ui, area: Rect, th: &Theme) {
        let now = ui.input(|i| i.time);
        match self.discard_armed_until {
            Some(t) if now < t => {}
            Some(_) => {
                self.discard_armed_until = None;
                return;
            }
            None => return,
        }
        let painter = ui
            .ctx()
            .layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("toast")));
        let galley = painter.layout_no_wrap(
            "Aperte esc de novo para descartar os rabiscos".to_owned(),
            FontId::proportional(14.0),
            th.toast_text,
        );
        let size = galley.size() + vec2(36.0, 20.0);
        let rect = Rect::from_center_size(pos2(area.center().x, area.min.y + 24.0), size);
        painter.rect_filled(rect, size.y / 2.0, th.toast_bg);
        painter.galley(rect.center() - galley.size() / 2.0, galley, th.toast_text);
    }
}

fn swatch(painter: &Painter, c: Pos2, r: f32, col: Color32, selected: bool, th: &Theme) {
    if selected {
        let ring = if th.swatch_needs_border(col) {
            th.swatch_border
        } else {
            col
        };
        painter.circle_stroke(c, r + 4.0, Stroke::new(2.0, ring));
    }
    painter.circle_filled(c, r, col);
    if th.swatch_needs_border(col) {
        painter.circle_stroke(c, r - 0.6, Stroke::new(1.2, th.swatch_border));
    }
}

fn icon_button(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    enabled: bool,
    tooltip: &str,
    th: &Theme,
    draw: impl FnOnce(&Painter, Pos2, Color32),
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, Id::new(id), sense).on_hover_text(tooltip);
    if enabled && resp.hovered() {
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, th.hover);
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    draw(
        ui.painter(),
        rect.center(),
        if enabled { th.icon } else { th.icon_disabled },
    );
    enabled && resp.clicked()
}

/// Xadrez para mostrar áreas transparentes da imagem.
fn checker_texture(ctx: &egui::Context) -> TextureHandle {
    let (a, b) = (Color32::from_gray(255), Color32::from_gray(228));
    let img = egui::ColorImage::new([2, 2], vec![a, b, b, a]);
    ctx.load_texture("rabisco-checker", img, TextureOptions::NEAREST_REPEAT)
}
