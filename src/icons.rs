//! Ícones da interface, desenhados em código com o `Painter` do egui, e o ícone
//! da barra de menus. O ícone do app fica em `assets/` (SVG + PNG).

use std::f32::consts::PI;

use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2, vec2};

/// Grid 24x24 (o mesmo dos ícones do Lucide) projetado num quadrado da tela.
struct Grid {
    center: Pos2,
    scale: f32,
}

impl Grid {
    fn new(center: Pos2, size: f32) -> Self {
        Self {
            center,
            scale: size / 24.0,
        }
    }

    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.center + vec2((x - 12.0) * self.scale, (y - 12.0) * self.scale)
    }

    fn v(&self, v: Vec2) -> Pos2 {
        self.p(v.x, v.y)
    }

    fn stroke(&self, color: Color32) -> Stroke {
        Stroke::new(1.9 * self.scale, color)
    }
}

/// Linha aberta com pontas arredondadas.
fn polyline(painter: &Painter, points: Vec<Pos2>, stroke: Stroke) {
    let r = stroke.width / 2.0;
    if let (Some(a), Some(b)) = (points.first(), points.last()) {
        painter.circle_filled(*a, r, stroke.color);
        painter.circle_filled(*b, r, stroke.color);
    }
    painter.add(Shape::line(points, stroke));
}

/// Contorno do lápis no grid 24x24: ponta embaixo à esquerda, borracha em cima à direita.
fn pencil_outline() -> (Vec<Vec2>, [Vec2; 2]) {
    let tip = vec2(4.0, 20.0);
    let end = vec2(18.6, 5.4);
    let d = (end - tip).normalized();
    let n = vec2(d.y, -d.x) * -1.0;
    let half = 2.5;
    let cone = 4.4;
    let base = tip + d * cone;

    let mut pts = vec![tip, base + n * half, end + n * half];
    for i in 1..8 {
        let a = i as f32 / 8.0 * PI;
        pts.push(end + (n * a.cos() + d * a.sin()) * half);
    }
    pts.push(end - n * half);
    pts.push(base - n * half);

    let band = end - d * 3.3;
    (pts, [band + n * half, band - n * half])
}

pub fn pencil(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let g = Grid::new(center, size);
    let stroke = g.stroke(color);
    let (outline, band) = pencil_outline();
    painter.add(Shape::closed_line(
        outline.iter().map(|v| g.v(*v)).collect(),
        stroke,
    ));
    painter.line_segment([g.v(band[0]), g.v(band[1])], stroke);
}

fn arc(cx: f32, cy: f32, r: f32, from_deg: f32, to_deg: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let t = from_deg + (to_deg - from_deg) * i as f32 / steps as f32;
            let a = t.to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Seta de desfazer (ou refazer, espelhada).
pub fn undo(painter: &Painter, center: Pos2, size: f32, color: Color32, mirrored: bool) {
    let g = Grid::new(center, size);
    let stroke = g.stroke(color);
    let fx = |x: f32| if mirrored { 24.0 - x } else { x };

    let head = vec![g.p(fx(9.0), 14.0), g.p(fx(4.0), 9.0), g.p(fx(9.0), 4.0)];
    polyline(painter, head, stroke);

    let mut body = vec![g.p(fx(4.0), 9.0), g.p(fx(14.5), 9.0)];
    for (x, y) in arc(14.5, 14.5, 5.5, -90.0, 90.0, 18).into_iter().skip(1) {
        body.push(g.p(fx(x), y));
    }
    body.push(g.p(fx(11.0), 20.0));
    polyline(painter, body, stroke);
}

pub fn check(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let g = Grid::new(center, size);
    let mut stroke = g.stroke(color);
    stroke.width *= 1.25;
    polyline(
        painter,
        vec![g.p(4.5, 12.5), g.p(9.5, 17.5), g.p(19.5, 7.0)],
        stroke,
    );
}

pub fn chevron(painter: &Painter, center: Pos2, size: f32, color: Color32, up: bool) {
    let g = Grid::new(center, size);
    let pts = if up {
        vec![g.p(6.0, 15.0), g.p(12.0, 9.0), g.p(18.0, 15.0)]
    } else {
        vec![g.p(6.0, 9.0), g.p(12.0, 15.0), g.p(18.0, 9.0)]
    };
    polyline(painter, pts, g.stroke(color));
}

/// Moldura de foto, usada no estado "nenhuma imagem".
pub fn image(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let g = Grid::new(center, size);
    let stroke = g.stroke(color);
    let rect = egui::Rect::from_min_max(g.p(3.0, 3.0), g.p(21.0, 21.0));
    painter.rect_stroke(rect, 2.0 * g.scale, stroke, egui::StrokeKind::Middle);
    painter.circle_stroke(g.p(9.0, 9.0), 2.0 * g.scale, stroke);
    polyline(
        painter,
        vec![g.p(21.0, 15.0), g.p(16.0, 10.0), g.p(5.0, 21.0)],
        stroke,
    );
}

/// Ícone "template" da barra de menus: a silhueta do cachorro de `assets/tray.svg`,
/// renderizada em `assets/tray.png` (36×36, 18 pt em telas Retina). O macOS usa só a
/// transparência e pinta de preto ou branco conforme o tema.
pub fn tray_icon() -> tray_icon::Icon {
    let image = image::load_from_memory(include_bytes!("../assets/tray.png"))
        .expect("assets/tray.png")
        .to_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).expect("ícone da barra de menus")
}
