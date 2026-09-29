//! O documento em edição: imagem original + traços do lápis.
//!
//! Os traços ficam guardados como vetores em pixels da imagem original. A tela mostra
//! uma cópia (possivelmente reduzida) que recebe os traços aos poucos enquanto o mouse
//! se move; ao salvar, tudo é redesenhado na resolução original com o mesmo código,
//! então o que se vê é o que vai para o clipboard.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::RgbaImage;
use tiny_skia::{
    Color, FillRule, IntSize, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapMut,
    Transform,
};

type Pt = [f32; 2];

fn mid(a: Pt, b: Pt) -> Pt {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

fn dist(a: Pt, b: Pt) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[derive(Clone)]
pub struct Stroke {
    /// RGBA sem pré-multiplicação.
    pub color: [u8; 4],
    /// Espessura em pixels da imagem original.
    pub width: f32,
    points: Vec<Pt>,
    min: Pt,
    max: Pt,
}

impl Stroke {
    fn new(p: Pt, color: [u8; 4], width: f32) -> Self {
        Self {
            color,
            width,
            points: vec![p],
            min: p,
            max: p,
        }
    }

    fn push(&mut self, p: Pt) {
        self.points.push(p);
        self.min = [self.min[0].min(p[0]), self.min[1].min(p[1])];
        self.max = [self.max[0].max(p[0]), self.max[1].max(p[1])];
    }

    /// Retângulo ocupado pelo traço, em pixels da imagem.
    fn bounds(&self) -> [f32; 4] {
        let pad = self.width / 2.0 + 2.0;
        [
            self.min[0] - pad,
            self.min[1] - pad,
            self.max[0] + pad,
            self.max[1] + pad,
        ]
    }

    /// Caminho suavizado: curvas quadráticas passando pelos pontos médios.
    fn path(&self) -> Option<Path> {
        let p = &self.points;
        let mut pb = PathBuilder::new();
        pb.move_to(p[0][0], p[0][1]);
        if p.len() == 2 {
            pb.line_to(p[1][0], p[1][1]);
        } else {
            let m = mid(p[0], p[1]);
            pb.line_to(m[0], m[1]);
            for i in 1..p.len() - 1 {
                let m = mid(p[i], p[i + 1]);
                pb.quad_to(p[i][0], p[i][1], m[0], m[1]);
            }
            let last = p[p.len() - 1];
            pb.line_to(last[0], last[1]);
        }
        pb.finish()
    }
}

fn paint(color: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
    paint.anti_alias = true;
    paint
}

fn pen(width: f32) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

fn draw_dot(pm: &mut PixmapMut, p: Pt, color: [u8; 4], width: f32, ts: Transform) {
    if let Some(circle) = PathBuilder::from_circle(p[0], p[1], width / 2.0) {
        pm.fill_path(&circle, &paint(color), FillRule::Winding, ts, None);
    }
}

fn draw_stroke(pm: &mut PixmapMut, s: &Stroke, ts: Transform) {
    if s.points.len() == 1 {
        draw_dot(pm, s.points[0], s.color, s.width, ts);
    } else if let Some(path) = s.path() {
        pm.stroke_path(&path, &paint(s.color), &pen(s.width), ts, None);
    }
}

/// Converte RGBA comum para o formato pré-multiplicado do tiny-skia.
fn to_pixmap(img: &RgbaImage) -> Pixmap {
    let mut data = img.as_raw().clone();
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3] as u16;
        if a != 255 {
            for c in &mut px[..3] {
                *c = ((*c as u16 * a + 127) / 255) as u8;
            }
        }
    }
    let size = IntSize::from_wh(img.width(), img.height()).expect("imagem sem tamanho");
    Pixmap::from_vec(data, size).expect("pixmap")
}

/// Retângulo inteiro em pixels da tela de desenho: [x0, y0, x1, y1).
type Region = [u32; 4];

fn union(a: Option<Region>, b: Region) -> Region {
    match a {
        None => b,
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    }
}

pub struct Document {
    base: RgbaImage,
    pub dpi: Option<[f64; 2]>,
    pub has_alpha: bool,
    strokes: Vec<Stroke>,
    redo: Vec<Stroke>,
    current: Option<Stroke>,
    /// Pixels de tela por pixel da imagem (menor que 1 quando a imagem é gigante).
    disp_scale: f32,
    base_disp: Pixmap,
    disp: Pixmap,
    dirty: Option<Region>,
    upload_all: bool,
    texture: Option<TextureHandle>,
}

impl Document {
    pub fn new(base: RgbaImage, dpi: Option<[f64; 2]>, max_display_side: u32) -> Self {
        let (w, h) = base.dimensions();
        let longest = w.max(h);
        let (disp_img, disp_scale) = if longest > max_display_side {
            let s = max_display_side as f32 / longest as f32;
            let dw = ((w as f32 * s).round() as u32).max(1);
            let dh = ((h as f32 * s).round() as u32).max(1);
            (
                image::imageops::thumbnail(&base, dw, dh),
                dw as f32 / w as f32,
            )
        } else {
            (base.clone(), 1.0)
        };
        let base_disp = to_pixmap(&disp_img);
        Self {
            has_alpha: base.pixels().any(|p| p[3] < 255),
            base,
            dpi,
            strokes: Vec::new(),
            redo: Vec::new(),
            current: None,
            disp_scale,
            disp: base_disp.clone(),
            base_disp,
            dirty: None,
            upload_all: true,
            texture: None,
        }
    }

    /// Largura e altura da imagem original.
    pub fn size(&self) -> [f32; 2] {
        [self.base.width() as f32, self.base.height() as f32]
    }

    pub fn is_edited(&self) -> bool {
        !self.strokes.is_empty() || self.current.is_some()
    }

    pub fn can_undo(&self) -> bool {
        !self.strokes.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn is_drawing(&self) -> bool {
        self.current.is_some()
    }

    /// Textura com o estado atual, enviando para a GPU só o que mudou.
    pub fn texture(&mut self, ctx: &Context) -> &TextureHandle {
        let options = TextureOptions::LINEAR;
        let (w, h) = (self.disp.width() as usize, self.disp.height() as usize);
        if self.texture.is_none() || self.upload_all {
            let img = ColorImage::from_rgba_premultiplied([w, h], self.disp.data());
            match &mut self.texture {
                Some(t) => t.set(img, options),
                None => self.texture = Some(ctx.load_texture("rabisco-canvas", img, options)),
            }
            self.upload_all = false;
            self.dirty = None;
        } else if let Some([x0, y0, x1, y1]) = self.dirty.take() {
            let (rw, rh) = ((x1 - x0) as usize, (y1 - y0) as usize);
            let data = self.disp.data();
            let mut region = Vec::with_capacity(rw * rh * 4);
            for y in y0 as usize..y1 as usize {
                let start = (y * w + x0 as usize) * 4;
                region.extend_from_slice(&data[start..start + rw * 4]);
            }
            let img = ColorImage::from_rgba_premultiplied([rw, rh], &region);
            if let Some(t) = &mut self.texture {
                t.set_partial([x0 as usize, y0 as usize], img, options);
            }
        }
        self.texture.as_ref().expect("textura criada acima")
    }

    fn display_transform(&self) -> Transform {
        Transform::from_scale(self.disp_scale, self.disp_scale)
    }

    /// Converte um retângulo em pixels da imagem para a tela de desenho, recortado nas bordas.
    fn to_region(&self, b: [f32; 4]) -> Option<Region> {
        let s = self.disp_scale;
        let (w, h) = (self.disp.width() as f32, self.disp.height() as f32);
        let x0 = (b[0] * s).floor().clamp(0.0, w) as u32;
        let y0 = (b[1] * s).floor().clamp(0.0, h) as u32;
        let x1 = (b[2] * s).ceil().clamp(0.0, w) as u32;
        let y1 = (b[3] * s).ceil().clamp(0.0, h) as u32;
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
    }

    fn mark(&mut self, bounds: [f32; 4]) {
        if let Some(r) = self.to_region(bounds) {
            self.dirty = Some(union(self.dirty, r));
        }
    }

    /// Começa um traço no ponto `p` (pixels da imagem).
    pub fn begin(&mut self, p: Pt, color: [u8; 4], width: f32) {
        self.end();
        let ts = self.display_transform();
        draw_dot(&mut self.disp.as_mut(), p, color, width, ts);
        let stroke = Stroke::new(p, color, width);
        self.mark(stroke.bounds());
        self.current = Some(stroke);
    }

    /// Estende o traço atual, desenhando só o pedaço novo.
    pub fn extend(&mut self, p: Pt) {
        let min_step = 0.75 / self.disp_scale;
        let ts = self.display_transform();
        let Some(s) = self.current.as_mut() else {
            return;
        };
        let last = s.points[s.points.len() - 1];
        if dist(last, p) < min_step {
            return;
        }
        s.push(p);
        let n = s.points.len();
        // O mesmo pedaço que `Stroke::path` gera entre dois pontos médios consecutivos.
        let (start, ctrl) = if n == 2 {
            (last, last)
        } else {
            (mid(s.points[n - 3], last), last)
        };
        let end = mid(last, p);
        let mut pb = PathBuilder::new();
        pb.move_to(start[0], start[1]);
        pb.quad_to(ctrl[0], ctrl[1], end[0], end[1]);
        let (color, width) = (s.color, s.width);
        if let Some(path) = pb.finish() {
            self.disp
                .stroke_path(&path, &paint(color), &pen(width), ts, None);
        }
        let pad = width / 2.0 + 2.0;
        let xs = [start[0], ctrl[0], end[0]];
        let ys = [start[1], ctrl[1], end[1]];
        let min = |v: [f32; 3]| v.into_iter().fold(f32::INFINITY, f32::min);
        let max = |v: [f32; 3]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
        self.mark([min(xs) - pad, min(ys) - pad, max(xs) + pad, max(ys) + pad]);
    }

    /// Trecho final do traço em andamento (do último ponto médio até o cursor),
    /// que a interface desenha por cima para o traço não "atrasar" em relação ao mouse.
    pub fn tail(&self) -> Option<(Pt, Pt, [u8; 4], f32)> {
        let s = self.current.as_ref()?;
        let n = s.points.len();
        (n >= 2).then(|| {
            (
                mid(s.points[n - 2], s.points[n - 1]),
                s.points[n - 1],
                s.color,
                s.width,
            )
        })
    }

    /// Termina o traço atual e o redesenha limpo (um caminho só, sem emendas).
    pub fn end(&mut self) {
        if let Some(stroke) = self.current.take() {
            let bounds = stroke.bounds();
            self.strokes.push(stroke);
            self.redo.clear();
            self.rerender(bounds);
        }
    }

    pub fn undo(&mut self) {
        self.end();
        if let Some(stroke) = self.strokes.pop() {
            let bounds = stroke.bounds();
            self.redo.push(stroke);
            self.rerender(bounds);
        }
    }

    pub fn redo(&mut self) {
        self.end();
        if let Some(stroke) = self.redo.pop() {
            let bounds = stroke.bounds();
            self.strokes.push(stroke);
            self.rerender(bounds);
        }
    }

    /// Refaz uma região da tela a partir da imagem original e dos traços.
    fn rerender(&mut self, bounds: [f32; 4]) {
        let Some([x0, y0, x1, y1]) = self.to_region(bounds) else {
            return;
        };
        let (rw, rh) = (x1 - x0, y1 - y0);
        let Some(mut tmp) = Pixmap::new(rw, rh) else {
            return;
        };
        let full_w = self.disp.width() as usize;
        let row = rw as usize * 4;
        for y in 0..rh as usize {
            let src = ((y0 as usize + y) * full_w + x0 as usize) * 4;
            tmp.data_mut()[y * row..(y + 1) * row]
                .copy_from_slice(&self.base_disp.data()[src..src + row]);
        }
        let ts = self
            .display_transform()
            .post_translate(-(x0 as f32), -(y0 as f32));
        let s = self.disp_scale;
        let (rx0, ry0, rx1, ry1) = (x0 as f32 / s, y0 as f32 / s, x1 as f32 / s, y1 as f32 / s);
        for stroke in &self.strokes {
            let b = stroke.bounds();
            if b[2] >= rx0 && b[0] <= rx1 && b[3] >= ry0 && b[1] <= ry1 {
                draw_stroke(&mut tmp.as_mut(), stroke, ts);
            }
        }
        let data = self.disp.data_mut();
        for y in 0..rh as usize {
            let dst = ((y0 as usize + y) * full_w + x0 as usize) * 4;
            data[dst..dst + row].copy_from_slice(&tmp.data()[y * row..(y + 1) * row]);
        }
        self.dirty = Some(union(self.dirty, [x0, y0, x1, y1]));
    }

    /// Imagem final, na resolução original.
    pub fn render(&self) -> RgbaImage {
        let mut pm = to_pixmap(&self.base);
        for stroke in self.strokes.iter().chain(self.current.iter()) {
            draw_stroke(&mut pm.as_mut(), stroke, Transform::identity());
        }
        RgbaImage::from_raw(
            self.base.width(),
            self.base.height(),
            pm.take_demultiplied(),
        )
        .expect("tamanho da imagem")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn stroke_is_rendered_and_undo_restores() {
        let mut doc = Document::new(white(100, 60), None, 4096);
        doc.begin([10.0, 30.0], [255, 0, 0, 255], 6.0);
        for x in 11..=90 {
            doc.extend([x as f32, 30.0]);
        }
        doc.end();
        let out = doc.render();
        assert_eq!(out.get_pixel(50, 30).0, [255, 0, 0, 255]);
        assert_eq!(out.get_pixel(50, 10).0, [255, 255, 255, 255]);

        doc.undo();
        assert_eq!(doc.render().get_pixel(50, 30).0, [255, 255, 255, 255]);
        doc.redo();
        assert_eq!(doc.render().get_pixel(50, 30).0, [255, 0, 0, 255]);
    }

    #[test]
    fn transparency_survives_the_round_trip() {
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([200, 100, 50, 128]));
        let doc = Document::new(img.clone(), None, 4096);
        assert!(doc.has_alpha);
        for (out, src) in doc.render().pixels().zip(img.pixels()) {
            for i in 0..4 {
                assert!(out[i].abs_diff(src[i]) <= 2, "{out:?} != {src:?}");
            }
        }
    }

    #[test]
    fn click_draws_a_dot() {
        let mut doc = Document::new(white(40, 40), None, 4096);
        doc.begin([20.0, 20.0], [0, 0, 255, 255], 8.0);
        doc.end();
        assert_eq!(doc.render().get_pixel(20, 20).0, [0, 0, 255, 255]);
    }

    #[test]
    fn display_matches_export_when_downscaled() {
        let mut doc = Document::new(white(2000, 1000), None, 1000);
        assert!((doc.disp_scale - 0.5).abs() < 1e-6);
        doc.begin([100.0, 500.0], [0, 128, 0, 255], 20.0);
        doc.extend([1900.0, 500.0]);
        doc.end();
        let px = doc.disp.pixel(500, 250).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (0, 128, 0));
        assert_eq!(doc.render().get_pixel(1000, 500).0, [0, 128, 0, 255]);
    }
}
