use std::fmt;

use crate::geometry::ExpandedPrimitive;
use crate::render::{self, RenderError};
use crate::scene::{Canvas, ClipSide, Fill};

const OUTSIDE_TOLERANCE_PX: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct OutsideWarning {
    pub source_path: String,
    pub id: Option<String>,
    pub repeat_index: Option<u16>,
    pub edge: ClipSide,
    pub excess_px: f64,
}
impl fmt::Display for OutsideWarning {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "WARN OUTSIDE path={}", self.source_path)?;
        if let Some(id) = &self.id {
            write!(output, " id={id}")?;
        }
        if let Some(index) = self.repeat_index {
            write!(output, " repeat_index={index}")?;
        }
        let edge = match self.edge {
            ClipSide::Left => "left",
            ClipSide::Top => "top",
            ClipSide::Right => "right",
            ClipSide::Bottom => "bottom",
        };
        write!(output, " edge={edge} excess_px={}", self.excess_px)
    }
}
fn exceeds_tolerance(excess: f64) -> bool {
    excess > OUTSIDE_TOLERANCE_PX
}
fn tight_bounds(path: &tiny_skia::Path, item: &ExpandedPrimitive) -> Result<[f64; 4], RenderError> {
    let bounds = path
        .compute_tight_bounds()
        .ok_or_else(|| render::raster_error(Some(item), "cannot compute tight path bounds"))?;
    let coordinates = [
        f64::from(bounds.left()),
        f64::from(bounds.top()),
        f64::from(bounds.right()),
        f64::from(bounds.bottom()),
    ];
    if coordinates.iter().any(|value| !value.is_finite()) {
        return Err(render::raster_error(
            Some(item),
            "tight path bounds are not finite",
        ));
    }
    Ok(coordinates)
}
fn primitive_bounds(item: &ExpandedPrimitive) -> Result<[f64; 4], RenderError> {
    let prepared = render::prepare_primitive(item)?;
    let mut outline = tight_bounds(&prepared.outline, item)?;
    if item.geometry.fill == Fill::None {
        return Ok(outline);
    }
    let fill = tight_bounds(&prepared.path, item)?;
    outline[0] = outline[0].min(fill[0]);
    outline[1] = outline[1].min(fill[1]);
    outline[2] = outline[2].max(fill[2]);
    outline[3] = outline[3].max(fill[3]);
    Ok(outline)
}
/// Checks pre-clipping painted bounds in drawing order without rendering or I/O.
pub fn check_bounds(
    canvas: Canvas,
    primitives: &[ExpandedPrimitive],
) -> Result<Vec<OutsideWarning>, RenderError> {
    let (width, height) = render::canvas_dimensions(canvas);
    let mut warnings = Vec::new();
    for item in primitives {
        let bounds = primitive_bounds(item)?;
        let sides = [
            (ClipSide::Left, -bounds[0]),
            (ClipSide::Top, -bounds[1]),
            (ClipSide::Right, bounds[2] - f64::from(width)),
            (ClipSide::Bottom, bounds[3] - f64::from(height)),
        ];
        for (edge, excess_px) in sides {
            if exceeds_tolerance(excess_px) && !item.allow_clip.contains(&edge) {
                warnings.push(OutsideWarning {
                    source_path: item.geometry.source_path.clone(),
                    id: item.geometry.id.clone(),
                    repeat_index: item.repeat_index,
                    edge,
                    excess_px,
                });
            }
        }
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests;
