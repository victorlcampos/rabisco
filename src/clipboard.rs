//! Leitura e escrita de imagens no clipboard do macOS (NSPasteboard).
//!
//! Ordem de leitura: arquivo de imagem copiado no Finder, PNG, TIFF e, por fim,
//! qualquer outro formato que o próprio AppKit saiba abrir (JPEG, HEIC, PDF...).
//! Na escrita vão PNG e TIFF, preservando o DPI (prints de tela Retina são 144 dpi,
//! e é isso que faz a imagem colar no tamanho certo no Notes, Keynote etc.).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSImage, NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSURL};

/// Imagem lida do clipboard, com a resolução (pontos por polegada) quando conhecida.
pub struct ClipImage {
    pub image: RgbaImage,
    pub dpi: Option<[f64; 2]>,
}

pub enum ReadError {
    /// O clipboard não tem imagem.
    NoImage,
    /// Há um arquivo copiado, mas ele não é uma imagem.
    NotAnImage(String),
    /// Havia uma imagem, mas não deu para decodificá-la.
    Decode(String),
}

impl ReadError {
    pub fn message(&self) -> String {
        match self {
            ReadError::NoImage => "Nenhuma imagem no clipboard".into(),
            ReadError::NotAnImage(name) => format!("\u{201c}{name}\u{201d} não é uma imagem"),
            ReadError::Decode(err) => format!("Não consegui abrir a imagem ({err})"),
        }
    }
}

/// Contador que o macOS incrementa a cada mudança no clipboard.
pub fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

pub fn read() -> Result<ClipImage, ReadError> {
    let pb = NSPasteboard::generalPasteboard();

    // O Finder coloca o ícone do arquivo como TIFF junto com a URL; por isso a URL vem primeiro.
    if let Some(path) = file_url(&pb) {
        return read_file(&path);
    }
    if let Some(data) = pb.dataForType(unsafe { NSPasteboardTypePNG }) {
        return decode(&data.to_vec()).map_err(ReadError::Decode);
    }
    if let Some(data) = pb.dataForType(unsafe { NSPasteboardTypeTIFF })
        && let Ok(img) = decode(&data.to_vec())
    {
        return Ok(img);
    }
    if NSImage::canInitWithPasteboard(&pb)
        && let Some(tiff) = NSImage::initWithPasteboard(NSImage::alloc(), &pb)
            .and_then(|img| img.TIFFRepresentation())
    {
        return decode(&tiff.to_vec()).map_err(ReadError::Decode);
    }
    Err(ReadError::NoImage)
}

fn file_url(pb: &NSPasteboard) -> Option<PathBuf> {
    let url = pb.stringForType(unsafe { NSPasteboardTypeFileURL })?;
    // A URL pode ser uma "file reference" (file:///.file/id=...); filePathURL resolve.
    let url = NSURL::URLWithString(&url)?;
    let path = url.filePathURL()?.path()?;
    Some(PathBuf::from(path.to_string()))
}

fn read_file(path: &Path) -> Result<ClipImage, ReadError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| ReadError::Decode(e.to_string()))?;
    if let Ok(img) = decode_oriented(&bytes) {
        return Ok(img);
    }
    // Formatos que só o macOS decodifica (HEIC, PDF, ...).
    let data = NSData::with_bytes(&bytes);
    if let Some(tiff) =
        NSImage::initWithData(NSImage::alloc(), &data).and_then(|i| i.TIFFRepresentation())
        && let Ok(img) = decode(&tiff.to_vec())
    {
        return Ok(img);
    }
    Err(ReadError::NotAnImage(name))
}

/// Decodifica aplicando a orientação EXIF (fotos de celular vêm "deitadas" sem isso).
fn decode_oriented(bytes: &[u8]) -> Result<ClipImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let format = reader.format();
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(ClipImage {
        image: img.to_rgba8(),
        dpi: dpi_of(bytes, format),
    })
}

fn decode(bytes: &[u8]) -> Result<ClipImage, String> {
    let format = image::guess_format(bytes).ok();
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    Ok(ClipImage {
        image: img.to_rgba8(),
        dpi: dpi_of(bytes, format),
    })
}

fn dpi_of(bytes: &[u8], format: Option<ImageFormat>) -> Option<[f64; 2]> {
    match format? {
        ImageFormat::Png => png_dpi(bytes),
        ImageFormat::Tiff => tiff_dpi(bytes),
        _ => None,
    }
}

/// Lê o chunk pHYs (pixels por metro) de um PNG.
fn png_dpi(bytes: &[u8]) -> Option<[f64; 2]> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        let kind = &bytes[i + 4..i + 8];
        if kind == b"IDAT" {
            return None;
        }
        if kind == b"pHYs" && len == 9 && i + 17 <= bytes.len() {
            let d = &bytes[i + 8..i + 17];
            let x = u32::from_be_bytes(d[0..4].try_into().ok()?);
            let y = u32::from_be_bytes(d[4..8].try_into().ok()?);
            return (d[8] == 1 && x > 0 && y > 0).then_some([x as f64 * 0.0254, y as f64 * 0.0254]);
        }
        i += 12 + len;
    }
    None
}

fn tiff_dpi(bytes: &[u8]) -> Option<[f64; 2]> {
    use tiff::decoder::{Decoder, ifd::Value};
    use tiff::tags::Tag;

    let mut dec = Decoder::new(Cursor::new(bytes)).ok()?;
    let rational = |v: Value| match v {
        Value::Rational(n, d) if n > 0 && d > 0 => Some(n as f64 / d as f64),
        _ => None,
    };
    let x = rational(dec.find_tag(Tag::XResolution).ok()??)?;
    let y = rational(dec.find_tag(Tag::YResolution).ok()??).unwrap_or(x);
    let unit = dec
        .find_tag(Tag::ResolutionUnit)
        .ok()
        .flatten()
        .and_then(|v| v.into_u16().ok());
    match unit {
        Some(3) => Some([x * 2.54, y * 2.54]),
        Some(1) => None,
        _ => Some([x, y]),
    }
}

/// Coloca a imagem no clipboard como PNG e TIFF.
pub fn write(img: &RgbaImage, dpi: Option<[f64; 2]>) -> Result<(), String> {
    let opaque = img.pixels().all(|p| p[3] == 255);
    let png = encode_png(img, opaque, dpi)?;
    let tiff = encode_tiff(img, opaque, dpi)?;

    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let wrote_png =
        pb.setData_forType(Some(&NSData::from_vec(png)), unsafe { NSPasteboardTypePNG });
    let wrote_tiff = pb.setData_forType(Some(&NSData::from_vec(tiff)), unsafe {
        NSPasteboardTypeTIFF
    });
    if wrote_png || wrote_tiff {
        Ok(())
    } else {
        Err("o macOS recusou a escrita no clipboard".into())
    }
}

fn rgb_bytes(img: &RgbaImage) -> Vec<u8> {
    img.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect()
}

fn encode_png(img: &RgbaImage, opaque: bool, dpi: Option<[f64; 2]>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width(), img.height());
        enc.set_color(if opaque {
            png::ColorType::Rgb
        } else {
            png::ColorType::Rgba
        });
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        if let Some([x, y]) = dpi {
            enc.set_pixel_dims(Some(png::PixelDimensions {
                xppu: (x / 0.0254).round() as u32,
                yppu: (y / 0.0254).round() as u32,
                unit: png::Unit::Meter,
            }));
        }
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        if opaque {
            writer.write_image_data(&rgb_bytes(img))
        } else {
            writer.write_image_data(img.as_raw())
        }
        .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn encode_tiff(img: &RgbaImage, opaque: bool, dpi: Option<[f64; 2]>) -> Result<Vec<u8>, String> {
    use tiff::encoder::{Rational, TiffEncoder, colortype};
    use tiff::tags::ResolutionUnit;

    let (w, h) = img.dimensions();
    let res = dpi.map(|[x, y]| {
        let r = |v: f64| Rational {
            n: (v * 100.0).round() as u32,
            d: 100,
        };
        (r(x), r(y))
    });
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut enc = TiffEncoder::new(&mut cursor).map_err(|e| e.to_string())?;
        macro_rules! write_image {
            ($color:ty, $data:expr) => {{
                let mut image = enc.new_image::<$color>(w, h).map_err(|e| e.to_string())?;
                if let Some((x, y)) = res {
                    image.resolution_unit(ResolutionUnit::Inch);
                    image.x_resolution(x);
                    image.y_resolution(y);
                }
                image.write_data($data).map_err(|e| e.to_string())?;
            }};
        }
        if opaque {
            write_image!(colortype::RGB8, &rgb_bytes(img));
        } else {
            write_image!(colortype::RGBA8, img.as_raw());
        }
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(alpha: u8) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(5, 3, image::Rgba([10, 20, 30, alpha]));
        img.put_pixel(2, 1, image::Rgba([200, 100, 50, alpha]));
        img
    }

    #[test]
    fn png_keeps_pixels_and_retina_dpi() {
        let img = sample(255);
        let back = decode(&encode_png(&img, true, Some([144.0, 144.0])).unwrap()).unwrap();
        assert_eq!(back.image, img);
        let [x, y] = back.dpi.unwrap();
        assert!((x - 144.0).abs() < 0.1 && (y - 144.0).abs() < 0.1);
    }

    #[test]
    fn tiff_keeps_alpha_and_dpi() {
        let img = sample(128);
        let back = decode(&encode_tiff(&img, false, Some([144.0, 144.0])).unwrap()).unwrap();
        assert_eq!(back.image, img);
        assert_eq!(back.dpi, Some([144.0, 144.0]));
    }

    #[test]
    fn png_without_dpi() {
        let img = sample(255);
        let back = decode(&encode_png(&img, true, None).unwrap()).unwrap();
        assert_eq!(back.dpi, None);
    }
}
