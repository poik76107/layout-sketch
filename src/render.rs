use std::fmt;

use crate::geometry::ExpandedPrimitive;
use crate::path_data::Segment;
use crate::scene::{Canvas, ErrorKind, Fill, Point, SceneError, Width};

const LANDSCAPE: (u32, u32) = (1536, 1024);
const PORTRAIT: (u32, u32) = (1024, 1536);
const SQUARE: (u32, u32) = (1024, 1024);
const THICK_WIDTH: f32 = 9.0;
const THIN_WIDTH: f32 = 4.5;
// tiny-skia 0.11.4 shifts scan Y by two before clipping. This f32 predecessor
// of 2^29 keeps that shift and the two-sided integer rectangle span in i32.
const BACKEND_LIMIT: f32 = 536_870_880.0;
const RASTER_EXTENT_LIMIT: f64 = 16_777_216.0;

#[derive(Debug)]
pub enum RenderError {
    InvalidGeometry(SceneError),
    Raster {
        source_path: Option<String>,
        repeat_index: Option<u16>,
        detail: String,
    },
}
impl fmt::Display for RenderError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry(error) => write!(output, "{error}"),
            Self::Raster {
                source_path,
                repeat_index,
                detail,
            } => {
                write!(output, "raster error")?;
                if let Some(path) = source_path {
                    write!(output, " path={path}")?;
                }
                if let Some(index) = repeat_index {
                    write!(output, " repeat_index={index}")?;
                }
                write!(output, ": {detail}")
            }
        }
    }
}
impl std::error::Error for RenderError {}

pub(crate) fn raster_error(item: Option<&ExpandedPrimitive>, detail: &str) -> RenderError {
    RenderError::Raster {
        source_path: item.map(|value| value.geometry.source_path.clone()),
        repeat_index: item.and_then(|value| value.repeat_index),
        detail: detail.into(),
    }
}
fn geometry_error(item: &ExpandedPrimitive, detail: &str) -> RenderError {
    let detail = match item.repeat_index {
        Some(index) => format!("{detail}; repeat_index={index}"),
        None => detail.into(),
    };
    RenderError::InvalidGeometry(SceneError::new(
        ErrorKind::Shape,
        &item.geometry.source_path,
        detail,
    ))
}
pub(crate) fn canvas_dimensions(canvas: Canvas) -> (u32, u32) {
    match canvas {
        Canvas::Landscape => LANDSCAPE,
        Canvas::Portrait => PORTRAIT,
        Canvas::Square => SQUARE,
    }
}
fn allocate_buffer(width: u32, height: u32, channels: usize) -> Result<Vec<u8>, RenderError> {
    let length = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| raster_error(None, "image byte length overflow"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| raster_error(None, "image allocation failed"))?;
    bytes.resize(length, 255);
    Ok(bytes)
}
/// Renders expanded pixel-space primitives to an opaque RGB8 main image.
pub fn render_main(
    canvas: Canvas,
    primitives: &[ExpandedPrimitive],
) -> Result<image::RgbImage, RenderError> {
    let (width, height) = canvas_dimensions(canvas);
    let size = tiny_skia::IntSize::from_wh(width, height)
        .ok_or_else(|| raster_error(None, "invalid canvas dimensions"))?;
    let rgba = allocate_buffer(width, height, 4)?;
    let mut pixmap = tiny_skia::Pixmap::from_vec(rgba, size)
        .ok_or_else(|| raster_error(None, "cannot create raster surface"))?;
    for item in primitives {
        paint_primitive(&mut pixmap, item)?;
    }
    let mut rgb = allocate_buffer(width, height, 3)?;
    let (rgba_pixels, _) = pixmap.data().as_chunks::<4>();
    let (rgb_pixels, _) = rgb.as_chunks_mut::<3>();
    for (source, target) in rgba_pixels.iter().zip(rgb_pixels.iter_mut()) {
        target.copy_from_slice(&source[..3]);
    }
    image::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| raster_error(None, "cannot create RGB image"))
}

pub(crate) fn render_preview(
    main: &image::RgbImage,
    scale: f64,
    grid: bool,
) -> Result<image::RgbImage, RenderError> {
    let width = (f64::from(main.width()) * scale + 0.5).floor() as u32;
    let height = (f64::from(main.height()) * scale + 0.5).floor() as u32;
    if width == 0 || height == 0 {
        return Err(raster_error(None, "invalid preview dimensions"));
    }
    let mut preview =
        image::imageops::resize(main, width, height, image::imageops::FilterType::Lanczos3);
    if grid {
        draw_grid(&mut preview, main.width(), main.height())?;
    }
    Ok(preview)
}

fn draw_grid(
    image: &mut image::RgbImage,
    main_width: u32,
    main_height: u32,
) -> Result<(), RenderError> {
    let (width, height) = image.dimensions();
    let mut rgba = allocate_buffer(width, height, 4)?;
    for (source, target) in image
        .as_raw()
        .as_chunks::<3>()
        .0
        .iter()
        .zip(rgba.as_chunks_mut::<4>().0.iter_mut())
    {
        target[..3].copy_from_slice(source);
    }
    let size = tiny_skia::IntSize::from_wh(width, height)
        .ok_or_else(|| raster_error(None, "invalid preview dimensions"))?;
    let mut pixmap = tiny_skia::Pixmap::from_vec(rgba, size)
        .ok_or_else(|| raster_error(None, "cannot create preview surface"))?;
    let mut paint = tiny_skia::Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color_rgba8(200, 200, 200, 255);
    let stroke = tiny_skia::Stroke {
        width: 1.0,
        ..Default::default()
    };
    for vertical in [true, false] {
        let main_dimension = if vertical { main_width } else { main_height };
        let preview_dimension = if vertical { width } else { height };
        let count = (f64::from(main_dimension) / 102.4).round() as u32;
        for index in 0..=count {
            let pos = (102.4 * f64::from(index) * f64::from(preview_dimension)
                / f64::from(main_dimension)) as f32;
            let mut builder = tiny_skia::PathBuilder::new();
            if vertical {
                builder.move_to(pos, 0.0);
                builder.line_to(pos, height as f32);
            } else {
                builder.move_to(0.0, pos);
                builder.line_to(width as f32, pos);
            }
            let path = builder
                .finish()
                .ok_or_else(|| raster_error(None, "cannot construct grid path"))?;
            pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    for (source, target) in pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(image.as_mut().as_chunks_mut::<3>().0.iter_mut())
    {
        target.copy_from_slice(&source[..3]);
    }
    Ok(())
}
fn convert_point(point: Point, item: &ExpandedPrimitive) -> Result<(f32, f32), RenderError> {
    let x = point[0] as f32;
    let y = point[1] as f32;
    if !x.is_finite() || !y.is_finite() {
        return Err(geometry_error(
            item,
            "coordinate cannot be represented as finite f32",
        ));
    }
    if x.abs() > BACKEND_LIMIT || y.abs() > BACKEND_LIMIT {
        return Err(geometry_error(
            item,
            "coordinate exceeds raster backend numeric range",
        ));
    }
    Ok((x, y))
}
fn build_path(item: &ExpandedPrimitive) -> Result<tiny_skia::Path, RenderError> {
    let mut builder = tiny_skia::PathBuilder::new();
    for segment in &item.geometry.segments {
        match *segment {
            Segment::Move(p) => {
                let (x, y) = convert_point(p, item)?;
                builder.move_to(x, y);
            }
            Segment::Line(p) => {
                let (x, y) = convert_point(p, item)?;
                builder.line_to(x, y);
            }
            Segment::Quadratic(a, b) => {
                let (x1, y1) = convert_point(a, item)?;
                let (x, y) = convert_point(b, item)?;
                builder.quad_to(x1, y1, x, y);
            }
            Segment::Cubic(a, b, c) => {
                let (x1, y1) = convert_point(a, item)?;
                let (x2, y2) = convert_point(b, item)?;
                let (x, y) = convert_point(c, item)?;
                builder.cubic_to(x1, y1, x2, y2, x, y);
            }
            Segment::Close => builder.close(),
        }
    }
    let path = builder
        .finish()
        .ok_or_else(|| raster_error(Some(item), "cannot construct raster path"))?;
    check_backend_path(&path, item)?;
    Ok(path)
}
fn check_backend_path(path: &tiny_skia::Path, item: &ExpandedPrimitive) -> Result<(), RenderError> {
    let bounds = path.bounds();
    if [bounds.left(), bounds.right(), bounds.top(), bounds.bottom()]
        .into_iter()
        .any(|v| !v.is_finite() || v.abs() > BACKEND_LIMIT)
    {
        return Err(geometry_error(
            item,
            "path exceeds raster backend numeric range",
        ));
    }
    Ok(())
}
fn check_path_extent(path: &tiny_skia::Path, item: &ExpandedPrimitive) -> Result<(), RenderError> {
    let bounds = path.bounds();
    let width = f64::from(bounds.right()).ceil() - f64::from(bounds.left()).floor();
    let height = f64::from(bounds.bottom()).ceil() - f64::from(bounds.top()).floor();
    if width > RASTER_EXTENT_LIMIT || height > RASTER_EXTENT_LIMIT {
        return Err(geometry_error(
            item,
            "path bounds exceed raster precision limit of 16777216 px; reduce the shape size or coordinate magnitudes",
        ));
    }
    Ok(())
}
pub(crate) struct PreparedPaths {
    pub(crate) path: tiny_skia::Path,
    pub(crate) outline: tiny_skia::Path,
}
pub(crate) fn prepare_primitive(item: &ExpandedPrimitive) -> Result<PreparedPaths, RenderError> {
    let path = build_path(item)?;
    let width = match item.geometry.width {
        Width::Thick => THICK_WIDTH,
        Width::Thin => THIN_WIDTH,
    };
    let stroke = tiny_skia::Stroke {
        width,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    let outline = path
        .stroke(&stroke, 1.0)
        .ok_or_else(|| raster_error(Some(item), "cannot construct stroke outline"))?;
    check_backend_path(&outline, item)?;
    check_path_extent(&outline, item)?;
    if item.geometry.fill != Fill::None {
        check_path_extent(&path, item)?;
    }
    Ok(PreparedPaths { path, outline })
}
fn paint_primitive(
    pixmap: &mut tiny_skia::Pixmap,
    item: &ExpandedPrimitive,
) -> Result<(), RenderError> {
    let PreparedPaths { path, outline } = prepare_primitive(item)?;
    let mut paint = tiny_skia::Paint {
        anti_alias: true,
        blend_mode: tiny_skia::BlendMode::SourceOver,
        ..Default::default()
    };
    match item.geometry.fill {
        Fill::None => {}
        Fill::White => {
            paint.set_color_rgba8(255, 255, 255, 255);
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::EvenOdd,
                tiny_skia::Transform::identity(),
                None,
            );
        }
        Fill::Black => {
            paint.set_color_rgba8(0, 0, 0, 255);
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::EvenOdd,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    paint.set_color_rgba8(0, 0, 0, 255);
    pixmap.fill_path(
        &outline,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests;
