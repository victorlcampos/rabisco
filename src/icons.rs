//! Ícones desenhados em código: os da interface (com o `Painter` do egui),
//! o da barra de menus e o ícone do app (rasterizados com tiny-skia).

use std::f32::consts::PI;

use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2, vec2};
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, Path, PathBuilder,
    Pixmap, Point, SpreadMode, Transform,
};

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

// ---------------------------------------------------------------------------
// Rasterizados com tiny-skia

fn path(points: &[(f32, f32)], close: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (x, y) = *points.first()?;
    pb.move_to(x, y);
    for &(x, y) in &points[1..] {
        pb.line_to(x, y);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(r, g, b, a));
    paint.anti_alias = true;
    paint
}

fn round_stroke(width: f32) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

/// Ícone "template" da barra de menus (o macOS pinta de preto ou branco).
pub fn tray_icon() -> tray_icon::Icon {
    const SIZE: u32 = 36;
    let mut pm = Pixmap::new(SIZE, SIZE).expect("pixmap do ícone");
    let scale = SIZE as f32 / 24.0;
    let ts = Transform::from_scale(scale, scale);
    let (outline, band) = pencil_outline();
    let outline: Vec<(f32, f32)> = outline.iter().map(|v| (v.x, v.y)).collect();
    let ink = solid(0, 0, 0, 255);
    let stroke = round_stroke(1.75);
    if let Some(p) = path(&outline, true) {
        pm.stroke_path(&p, &ink, &stroke, ts, None);
    }
    if let Some(p) = path(&[(band[0].x, band[0].y), (band[1].x, band[1].y)], false) {
        pm.stroke_path(&p, &ink, &stroke, ts, None);
    }
    tray_icon::Icon::from_rgba(pm.take_demultiplied(), SIZE, SIZE).expect("ícone da barra de menus")
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Ícone do app (quadrado verde com um lápis e um rabisco), como PNG.
pub fn app_icon_png(size: u32) -> Vec<u8> {
    let mut pm = Pixmap::new(size, size).expect("pixmap do ícone");
    let s = size as f32 / 1024.0;
    let canvas = Transform::from_scale(s, s);

    // Sombra suave sob o "squircle".
    for i in 0..12 {
        let spread = i as f32 * 2.5;
        let alpha = (14.0 * (1.0 - i as f32 / 12.0)) as u8;
        if let Some(p) = rounded_rect(
            100.0 - spread,
            112.0 - spread,
            824.0 + 2.0 * spread,
            824.0 + 2.0 * spread,
            185.0 + spread,
        ) {
            pm.fill_path(&p, &solid(0, 0, 0, alpha), FillRule::Winding, canvas, None);
        }
    }

    let body = rounded_rect(100.0, 100.0, 824.0, 824.0, 185.0).expect("fundo do ícone");
    let gradient = LinearGradient::new(
        Point::from_xy(512.0, 100.0),
        Point::from_xy(512.0, 924.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(0x4c, 0xe0, 0x8b, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0x0e, 0x9a, 0x4d, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .expect("gradiente");
    let paint = Paint {
        shader: gradient,
        anti_alias: true,
        ..Default::default()
    };
    pm.fill_path(&body, &paint, FillRule::Winding, canvas, None);

    // O rabisco, terminando na ponta do lápis.
    let mut pb = PathBuilder::new();
    pb.move_to(215.0, 735.0);
    pb.cubic_to(250.0, 655.0, 330.0, 650.0, 315.0, 735.0);
    pb.cubic_to(300.0, 820.0, 390.0, 800.0, 440.0, 690.0);
    if let Some(p) = pb.finish() {
        pm.stroke_path(
            &p,
            &solid(255, 255, 255, 240),
            &round_stroke(44.0),
            canvas,
            None,
        );
    }

    // Lápis desenhado na horizontal (ponta em x = 0) e girado 45°.
    let pencil = Transform::from_rotate(-45.0)
        .post_translate(440.0, 690.0)
        .post_scale(s, s);
    let len = 480.0;
    let half = 72.0;
    let cone = 125.0;
    let lead = 42.0;
    let fill = |pm: &mut Pixmap, pts: &[(f32, f32)], rgb: (u8, u8, u8)| {
        if let Some(p) = path(pts, true) {
            pm.fill_path(
                &p,
                &solid(rgb.0, rgb.1, rgb.2, 255),
                FillRule::Winding,
                pencil,
                None,
            );
        }
    };
    let lead_half = half * lead / cone;
    let ferrule = len - 110.0;
    fill(
        &mut pm,
        &[(0.0, 0.0), (cone, -half), (cone, half)],
        (0xf7, 0xd9, 0xa8),
    );
    fill(
        &mut pm,
        &[(0.0, 0.0), (lead, -lead_half), (lead, lead_half)],
        (0x26, 0x32, 0x38),
    );
    fill(
        &mut pm,
        &[
            (cone, -half),
            (ferrule, -half),
            (ferrule, half),
            (cone, half),
        ],
        (0xff, 0xff, 0xff),
    );
    fill(
        &mut pm,
        &[(cone, 22.0), (ferrule, 22.0), (ferrule, half), (cone, half)],
        (0xe3, 0xea, 0xee),
    );
    // Borracha arredondada; a ponteira de metal, desenhada por cima, esconde o lado esquerdo.
    if let Some(p) = rounded_rect(ferrule, -half, len - ferrule, 2.0 * half, 30.0) {
        pm.fill_path(
            &p,
            &solid(0xff, 0x8a, 0x80, 255),
            FillRule::Winding,
            pencil,
            None,
        );
    }
    fill(
        &mut pm,
        &[
            (ferrule, -half),
            (len - 72.0, -half),
            (len - 72.0, half),
            (ferrule, half),
        ],
        (0xb0, 0xbe, 0xc5),
    );

    encode_png(&pm)
}

fn encode_png(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, pm.width(), pm.height());
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("cabeçalho PNG");
        writer
            .write_image_data(&pm.clone().take_demultiplied())
            .expect("dados PNG");
    }
    out
}
