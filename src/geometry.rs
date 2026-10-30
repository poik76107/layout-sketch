use crate::path_data::{Segment, parse_path};
use crate::scene::{
    ClipSide, ErrorKind, Fill, Line, Node, Point, Primitive, Scene, SceneError, Shape, Style,
    field_path,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
#[derive(Debug, PartialEq)]
pub struct Geometry {
    pub segments: Vec<Segment>,
    pub bounds: Bounds,
    pub width: crate::scene::Width,
    pub fill: Fill,
    pub id: Option<String>,
    pub source_path: String,
    pub fill_rule: tiny_skia::FillRule,
}
fn error(path: &str, field: &str, detail: &str) -> SceneError {
    let address = if field.is_empty() {
        path.into()
    } else {
        field_path(path, field)
    };
    SceneError::new(ErrorKind::Shape, &address, detail)
}
fn checked_point(point: Point, path: &str, field: &str) -> Result<Point, SceneError> {
    if point
        .iter()
        .any(|value| !value.is_finite() || !(value * 10.24).is_finite())
    {
        return Err(error(
            path,
            field,
            "computed coordinate is not finite at canvas scale",
        ));
    }
    Ok(point)
}
fn direction(angle: f64) -> (f64, f64) {
    bounded_sin_cos(angle.rem_euclid(360.0).to_radians())
}
fn bounded_sin_cos(angle: f64) -> (f64, f64) {
    let mut x = angle.rem_euclid(std::f64::consts::TAU);
    if x > std::f64::consts::PI {
        x -= std::f64::consts::TAU;
    }
    let square = x * x;
    let mut sin = x;
    let mut cos = 1.0;
    let mut sin_term = x;
    let mut cos_term = 1.0;
    for index in 1..=20 {
        let n = index as f64;
        sin_term *= -square / ((2.0 * n) * (2.0 * n + 1.0));
        cos_term *= -square / ((2.0 * n - 1.0) * (2.0 * n));
        sin += sin_term;
        cos += cos_term;
    }
    (sin, cos)
}
fn polar_end(from: Point, angle: f64, length: f64, path: &str) -> Result<Point, SceneError> {
    let (sin, cos) = direction(angle);
    checked_point(
        [from[0] + length * cos, from[1] + length * sin],
        path,
        "length",
    )
}
fn check_ellipse(
    center: Point,
    rx: f64,
    ry: f64,
    angle: f64,
    path: &str,
) -> Result<(), SceneError> {
    checked_point(center, path, "")?;
    let (sin, cos) = direction(angle);
    let extent_x = (rx * cos).hypot(ry * sin);
    let extent_y = (rx * sin).hypot(ry * cos);
    checked_point([center[0] + extent_x, center[1] + extent_y], path, "")?;
    checked_point([center[0] - extent_x, center[1] - extent_y], path, "")?;
    Ok(())
}
pub(crate) fn validate_finite(primitive: &Primitive, path: &str) -> Result<(), SceneError> {
    match primitive {
        Primitive::Line(Line::Endpoints { from, to }) => {
            checked_point(*from, path, "from")?;
            checked_point(*to, path, "to")?;
        }
        Primitive::Line(Line::Polar {
            from,
            angle,
            length,
        }) => {
            checked_point(*from, path, "from")?;
            polar_end(*from, *angle, *length, path)?;
        }
        Primitive::Polyline { pts } | Primitive::Polygon { pts } => {
            for (index, point) in pts.iter().enumerate() {
                checked_point(*point, &format!("{path}.pts[{index}]"), "")?;
            }
        }
        Primitive::Rect { x, y, w, h } => {
            checked_point([*x, *y], path, "")?;
            checked_point([x + w, y + h], path, "")?;
        }
        Primitive::Circle { cx, cy, r } => check_ellipse([*cx, *cy], *r, *r, 0.0, path)?,
        Primitive::Ellipse {
            cx,
            cy,
            rx,
            ry,
            angle,
        }
        | Primitive::Arc {
            cx,
            cy,
            rx,
            ry,
            angle,
            ..
        } => check_ellipse([*cx, *cy], *rx, *ry, *angle, path)?,
        Primitive::Curve {
            from,
            ctrl1,
            ctrl2,
            to,
        } => {
            for point in [from, ctrl1, ctrl2, to] {
                checked_point(*point, path, "")?;
            }
        }
        Primitive::Path { d } => {
            let parsed = parse_path(d).map_err(|failure| {
                let mut result = error(path, "d", &failure.detail);
                result.kind = ErrorKind::InvalidPath;
                result.offset = Some(failure.offset);
                result
            })?;
            for segment in &parsed.segments {
                validate_segment(segment, path)?;
            }
        }
    }
    Ok(())
}
fn validate_segment(segment: &Segment, path: &str) -> Result<(), SceneError> {
    match segment {
        Segment::Move(p) | Segment::Line(p) => {
            checked_point(*p, path, "d")?;
        }
        Segment::Quadratic(a, b) => {
            checked_point(*a, path, "d")?;
            checked_point(*b, path, "d")?;
        }
        Segment::Cubic(a, b, c) => {
            checked_point(*a, path, "d")?;
            checked_point(*b, path, "d")?;
            checked_point(*c, path, "d")?;
        }
        Segment::Close => {}
    }
    Ok(())
}

pub fn build_primitive(
    primitive: &Primitive,
    style: &Style,
    source_path: &str,
    id: Option<&str>,
    final_scale: f64,
) -> Result<Geometry, SceneError> {
    if !final_scale.is_finite() || final_scale <= 0.0 {
        return Err(error(
            source_path,
            "",
            "final scale must be positive and finite",
        ));
    }
    validate_finite(primitive, source_path)?;
    let segments = primitive_segments(primitive, source_path, final_scale)?;
    let bounds = calculate_bounds(&segments, source_path)?;
    Ok(Geometry {
        segments,
        bounds,
        width: style.width,
        fill: style.fill,
        id: id.map(str::to_owned),
        source_path: source_path.into(),
        fill_rule: tiny_skia::FillRule::EvenOdd,
    })
}
/// A primitive in final pixels, with geometric bounds excluding stroke.
#[derive(Debug, PartialEq)]
pub struct ExpandedPrimitive {
    pub geometry: Geometry,
    pub repeat_index: Option<u16>,
    pub allow_clip: Vec<ClipSide>,
}

#[derive(Clone, Copy)]
struct GroupTransform {
    at: Point,
    rotate: f64,
    scale: f64,
    flip_x: bool,
}
#[derive(Clone, Copy)]
struct RepeatTransform {
    center: Point,
    angle: f64,
    phase_error_degrees: f64,
    index: u16,
}

#[derive(Clone, Copy)]
struct ExpansionContext<'a> {
    group: Option<GroupTransform>,
    repeat: Option<RepeatTransform>,
    clips: &'a [ClipSide],
}

#[derive(Clone, Copy)]
struct RepeatDefinition {
    center: Point,
    count: u16,
    start: f64,
    step: Option<f64>,
}

/// Expands a validated scene in drawing order; segment coordinates and bounds are pixels.
pub fn expand_scene(scene: &Scene) -> Result<Vec<ExpandedPrimitive>, SceneError> {
    crate::validate::validate_scene(scene)?;
    let mut result = Vec::new();
    let context = ExpansionContext {
        group: None,
        repeat: None,
        clips: &[],
    };
    expand_nodes(&scene.shapes, "shapes", context, &mut result)?;
    Ok(result)
}
fn expand_nodes(
    nodes: &[Node],
    path: &str,
    context: ExpansionContext<'_>,
    result: &mut Vec<ExpandedPrimitive>,
) -> Result<(), SceneError> {
    for (index, node) in nodes.iter().enumerate() {
        let node_path = format!("{path}[{index}]");
        expand_node(node, &node_path, context, result)?;
    }
    Ok(())
}
fn add_clip_sides(inherited: &[ClipSide], own: &[ClipSide]) -> Vec<ClipSide> {
    let mut clips = inherited.to_vec();
    for side in own {
        if !clips.contains(side) {
            clips.push(*side);
        }
    }
    clips
}
fn expand_node(
    node: &Node,
    path: &str,
    context: ExpansionContext<'_>,
    result: &mut Vec<ExpandedPrimitive>,
) -> Result<(), SceneError> {
    let clips = add_clip_sides(context.clips, &node.allow_clip);
    let context = ExpansionContext {
        clips: &clips,
        ..context
    };
    match &node.shape {
        Shape::Primitive { primitive, style } => {
            expand_primitive_node(primitive, style, node.id.as_deref(), path, context, result)
        }
        Shape::Group {
            at,
            rotate,
            scale,
            flip_x,
            shapes,
        } => {
            let group = GroupTransform {
                at: *at,
                rotate: *rotate,
                scale: *scale,
                flip_x: *flip_x,
            };
            expand_group_node(shapes, group, path, context, result)
        }
        Shape::RepeatRotate {
            center,
            count,
            start,
            step,
            shapes,
        } => {
            let repeat = RepeatDefinition {
                center: *center,
                count: *count,
                start: *start,
                step: *step,
            };
            expand_repeat_node(shapes, repeat, path, context, result)
        }
    }
}
fn expand_primitive_node(
    primitive: &Primitive,
    style: &Style,
    id: Option<&str>,
    path: &str,
    context: ExpansionContext<'_>,
    result: &mut Vec<ExpandedPrimitive>,
) -> Result<(), SceneError> {
    let scale = context.group.map_or(1.0, |value| value.scale) * 10.24;
    let mut geometry = build_primitive(primitive, style, path, id, scale)
        .map_err(|failure| copy_error(failure, context.repeat))?;
    let accuracy = match primitive {
        Primitive::Circle { .. } | Primitive::Ellipse { .. } | Primitive::Arc { .. } => {
            Accuracy::Ellipse
        }
        _ => Accuracy::Other,
    };
    let transform = PointTransform {
        group: context.group,
        repeat: context.repeat,
        path,
        accuracy,
    };
    for segment in &mut geometry.segments {
        transform_segment(segment, transform)?;
    }
    geometry.bounds = calculate_bounds(&geometry.segments, path)
        .map_err(|failure| copy_error(failure, context.repeat))?;
    result
        .try_reserve(1)
        .map_err(|_| error(path, "", "expanded primitive allocation failed"))?;
    result.push(ExpandedPrimitive {
        geometry,
        repeat_index: context.repeat.map(|value| value.index),
        allow_clip: context.clips.to_vec(),
    });
    Ok(())
}
fn expand_group_node(
    shapes: &[Node],
    group: GroupTransform,
    path: &str,
    context: ExpansionContext<'_>,
    result: &mut Vec<ExpandedPrimitive>,
) -> Result<(), SceneError> {
    if !(10.24 * group.scale).is_finite() {
        return Err(error(path, "scale", "final scale is not finite"));
    }
    let context = ExpansionContext {
        group: Some(group),
        ..context
    };
    expand_nodes(shapes, &field_path(path, "shapes"), context, result)
}
fn repeat_angle(
    repeat: RepeatDefinition,
    index: u16,
    path: &str,
) -> Result<(f64, f64), SceneError> {
    let step = repeat.step.unwrap_or(360.0 / f64::from(repeat.count));
    let delta = f64::from(index) * step;
    if !delta.is_finite() {
        return Err(error(
            path,
            "step",
            &format!("repeat angle product is not finite; repeat_index={index}"),
        ));
    }
    let angle = repeat.start + delta;
    if !angle.is_finite() {
        return Err(error(
            path,
            "start",
            &format!("repeat angle sum is not finite; repeat_index={index}"),
        ));
    }
    // FMA recovers the product residual in one rounding, including a possible
    // subnormal residual. The sum's absolute rounding error is bounded by one
    // epsilon of its rounded magnitude plus MIN_POSITIVE. When either addend
    // is zero the sum is exact. For the default step, account separately for
    // rounding the division 360/count before multiplying by the exact index.
    let product_error = f64::from(index).mul_add(step, -delta).abs() + f64::MIN_POSITIVE;
    let sum_error = if repeat.start == 0.0 || delta == 0.0 {
        0.0
    } else {
        angle.abs() * f64::EPSILON + f64::MIN_POSITIVE
    };
    let default_error = if repeat.step.is_none() {
        (step.abs() * f64::EPSILON + f64::MIN_POSITIVE) * f64::from(index)
    } else {
        0.0
    };
    // The factor two and absolute term cover rounding in this upper-bound
    // computation itself, including a residual rounded into the subnormals.
    let phase_error_degrees = (product_error + sum_error + default_error) * 2.0 + f64::MIN_POSITIVE;
    Ok((angle, phase_error_degrees))
}
fn expand_repeat_node(
    shapes: &[Node],
    repeat: RepeatDefinition,
    path: &str,
    context: ExpansionContext<'_>,
    result: &mut Vec<ExpandedPrimitive>,
) -> Result<(), SceneError> {
    for index in 0..repeat.count {
        let (angle, phase_error_degrees) = repeat_angle(repeat, index, path)?;
        let transform = RepeatTransform {
            center: repeat.center,
            angle,
            phase_error_degrees,
            index,
        };
        let context = ExpansionContext {
            repeat: Some(transform),
            ..context
        };
        expand_nodes(shapes, &field_path(path, "shapes"), context, result)?;
    }
    Ok(())
}

fn copy_error(mut failure: SceneError, repeat: Option<RepeatTransform>) -> SceneError {
    if let Some(repeat) = repeat {
        failure
            .detail
            .push_str(&format!("; repeat_index={}", repeat.index));
    }
    failure
}
#[derive(Clone, Copy)]
enum Accuracy {
    Ellipse,
    Other,
}
#[derive(Clone, Copy)]
struct PointTransform<'a> {
    group: Option<GroupTransform>,
    repeat: Option<RepeatTransform>,
    path: &'a str,
    accuracy: Accuracy,
}
struct PointMagnitude {
    point: Point,
    magnitude: f64,
}
fn transform_point(point: Point, transform: PointTransform<'_>) -> Result<Point, SceneError> {
    let state = PointMagnitude {
        point,
        magnitude: point[0].abs().max(point[1].abs()),
    };
    let state = apply_repeat(state, transform)?;
    let state = apply_group(state, transform)?;
    let magnitude = state
        .magnitude
        .max(state.point[0].abs())
        .max(state.point[1].abs());
    check_map_accuracy(magnitude, point, transform)?;
    finite_transform(
        [state.point[0] * 10.24, state.point[1] * 10.24],
        transform.path,
        transform.repeat,
    )
}
fn apply_repeat(
    state: PointMagnitude,
    transform: PointTransform<'_>,
) -> Result<PointMagnitude, SceneError> {
    let Some(repeat) = transform.repeat else {
        return Ok(state);
    };
    let (sin, cos) = direction(repeat.angle);
    let point = [
        state.point[0] * cos - state.point[1] * sin,
        state.point[0] * sin + state.point[1] * cos,
    ];
    let point = finite_transform(point, transform.path, Some(repeat))?;
    let magnitude = state
        .magnitude
        .max(point[0].abs())
        .max(point[1].abs())
        .max(repeat.center[0].abs())
        .max(repeat.center[1].abs());
    let point = [point[0] + repeat.center[0], point[1] + repeat.center[1]];
    let point = finite_transform(point, transform.path, Some(repeat))?;
    Ok(PointMagnitude { point, magnitude })
}
fn apply_group(
    state: PointMagnitude,
    transform: PointTransform<'_>,
) -> Result<PointMagnitude, SceneError> {
    let Some(group) = transform.group else {
        return Ok(state);
    };
    let x = if group.flip_x {
        -state.point[0]
    } else {
        state.point[0]
    };
    let point = [x * group.scale, state.point[1] * group.scale];
    let point = finite_transform(point, transform.path, transform.repeat)?;
    let magnitude = (state.magnitude * group.scale)
        .max(point[0].abs())
        .max(point[1].abs());
    let (sin, cos) = direction(group.rotate);
    let point = [
        point[0] * cos - point[1] * sin,
        point[0] * sin + point[1] * cos,
    ];
    let point = finite_transform(point, transform.path, transform.repeat)?;
    let magnitude = magnitude
        .max(point[0].abs())
        .max(point[1].abs())
        .max(group.at[0].abs())
        .max(group.at[1].abs());
    let point = [point[0] + group.at[0], point[1] + group.at[1]];
    let point = finite_transform(point, transform.path, transform.repeat)?;
    Ok(PointMagnitude { point, magnitude })
}
fn check_map_accuracy(
    magnitude: f64,
    local_point: Point,
    transform: PointTransform<'_>,
) -> Result<(), SceneError> {
    if matches!(transform.accuracy, Accuracy::Other) {
        return Ok(());
    }
    // Let M be the maximum absolute component of each map operand or result,
    // expressed after group scaling but before pixel conversion. It includes
    // both addends separately, so cancellation cannot hide a large operand.
    // Each rotation has four products and two sums; each translation has two
    // sums. Reflection is exact, scaling and pixel conversion each add one
    // rounding per component. For binary64, |fl(x op y) - (x op y)| is at
    // most EPSILON times the sum of operand magnitudes plus MIN_POSITIVE.
    // The 20-term bounded sine/cosine recurrence uses fewer than 200 basic
    // operations with terms bounded by exp(pi); its error is below 512*EPSILON,
    // including degree reduction and the Taylor remainder (<1e-27). Two
    // rotations preserve Euclidean length. Each rotation's two trig errors
    // contribute at most 2*sqrt(2)*512 epsilon times M; both contribute less
    // than 2897 epsilon M. Their products and sums, two translations, group
    // scale and pixel conversion together contribute less than 128 epsilon M;
    // amplification of earlier rounding and conversion to Euclidean distance
    // fit in the remaining 1071 epsilon M. Thus 4096 epsilon M*10.24 bounds
    // map evaluation, separate from phase error. Linear interpolation is a convex combination
    // of endpoint errors and adds no second endpoint-error factor.
    // Include an absolute underflow allowance for every operation before and
    // after group scaling. The 0.0125 px map allowance is separate from the
    // subdivision's 0.025 px allowance; the local interpolation target below
    // is 0.085 px, so 0.085 + 0.125 + 0.025 + 0.0125 < 0.25 px.
    let group_scale = transform.group.map_or(1.0, |value| value.scale);
    let map_rounding = (magnitude * f64::EPSILON * 10.24
        + f64::MIN_POSITIVE * (group_scale * 10.24 + 10.24))
        * 4096.0;
    // Rotating a local point by a phase error d moves it by at most r*|d|
    // radians. Reflection and rotation preserve distance; group scale and
    // 10.24 convert r to pixels. The local distance from the repeat origin is
    // <= sqrt(2)*max(|x|,|y|). 0.04 exceeds sqrt(2)*pi/180 with room for
    // rounding of these products; MIN_POSITIVE covers underflow. This phase
    // allowance and map evaluation share the 0.0125 px budget. Capping the
    // error at 360 degrees is safe: 0.04*360 exceeds the maximum chord
    // displacement 2*sqrt(2)*max(|x|,|y|), even for many full turns.
    let phase_degrees = transform
        .repeat
        .map_or(0.0, |repeat| repeat.phase_error_degrees);
    let radius = local_point[0].abs().max(local_point[1].abs());
    let scaled_radius = radius * (group_scale * 10.24);
    let phase_rounding = scaled_radius * phase_degrees.min(360.0) * 0.04 + f64::MIN_POSITIVE * 16.0;
    let total_rounding = map_rounding + phase_rounding;
    if !total_rounding.is_finite() || total_rounding >= 0.0125 {
        return Err(copy_error(
            error(
                transform.path,
                "",
                "ellipse transform accuracy cannot be represented",
            ),
            transform.repeat,
        ));
    }
    Ok(())
}

fn finite_transform(
    point: Point,
    path: &str,
    repeat: Option<RepeatTransform>,
) -> Result<Point, SceneError> {
    if point.iter().any(|value| !value.is_finite()) {
        return Err(copy_error(
            error(path, "", "transformed coordinate is not finite"),
            repeat,
        ));
    }
    Ok(point)
}
fn transform_segment(
    segment: &mut Segment,
    transform: PointTransform<'_>,
) -> Result<(), SceneError> {
    match segment {
        Segment::Move(p) | Segment::Line(p) => *p = transform_point(*p, transform)?,
        Segment::Quadratic(a, b) => {
            *a = transform_point(*a, transform)?;
            *b = transform_point(*b, transform)?;
        }
        Segment::Cubic(a, b, c) => {
            *a = transform_point(*a, transform)?;
            *b = transform_point(*b, transform)?;
            *c = transform_point(*c, transform)?;
        }
        Segment::Close => {}
    }
    Ok(())
}

fn primitive_segments(
    primitive: &Primitive,
    path: &str,
    scale: f64,
) -> Result<Vec<Segment>, SceneError> {
    let segments = match primitive {
        Primitive::Line(Line::Endpoints { from, to }) => {
            vec![Segment::Move(*from), Segment::Line(*to)]
        }
        Primitive::Line(Line::Polar {
            from,
            angle,
            length,
        }) => vec![
            Segment::Move(*from),
            Segment::Line(polar_end(*from, *angle, *length, path)?),
        ],
        Primitive::Polyline { pts } => points_path(pts, false),
        Primitive::Polygon { pts } => {
            let points = if pts.len() > 1 && pts.first() == pts.last() {
                &pts[..pts.len() - 1]
            } else {
                pts
            };
            points_path(points, true)
        }
        Primitive::Rect { x, y, w, h } => vec![
            Segment::Move([*x, *y]),
            Segment::Line([x + w, *y]),
            Segment::Line([x + w, y + h]),
            Segment::Line([*x, y + h]),
            Segment::Close,
        ],
        Primitive::Curve {
            from,
            ctrl1,
            ctrl2,
            to,
        } => vec![Segment::Move(*from), Segment::Cubic(*ctrl1, *ctrl2, *to)],
        Primitive::Path { d } => {
            parse_path(d)
                .map_err(|failure| {
                    let mut result = error(path, "d", &failure.detail);
                    result.kind = ErrorKind::InvalidPath;
                    result.offset = Some(failure.offset);
                    result
                })?
                .segments
        }
        Primitive::Circle { cx, cy, r } => ellipse_path(
            Ellipse {
                center: [*cx, *cy],
                rx: *r,
                ry: *r,
                angle: 0.0,
                start: 0.0,
                sweep: 360.0,
                close: true,
            },
            path,
            scale,
        )?,
        Primitive::Ellipse {
            cx,
            cy,
            rx,
            ry,
            angle,
        } => ellipse_path(
            Ellipse {
                center: [*cx, *cy],
                rx: *rx,
                ry: *ry,
                angle: *angle,
                start: 0.0,
                sweep: 360.0,
                close: true,
            },
            path,
            scale,
        )?,
        Primitive::Arc {
            cx,
            cy,
            rx,
            ry,
            angle,
            start,
            sweep,
            chord,
        } => ellipse_path(
            Ellipse {
                center: [*cx, *cy],
                rx: *rx,
                ry: *ry,
                angle: *angle,
                start: *start,
                sweep: *sweep,
                close: *chord,
            },
            path,
            scale,
        )?,
    };
    Ok(segments)
}
fn points_path(points: &[Point], close: bool) -> Vec<Segment> {
    let mut segments = Vec::new();
    if let Some((first, rest)) = points.split_first() {
        segments.push(Segment::Move(*first));
        for point in rest {
            segments.push(Segment::Line(*point));
        }
        if close {
            segments.push(Segment::Close);
        }
    }
    segments
}
fn ellipse_point(center: Point, rx: f64, ry: f64, angle: f64, parameter: f64) -> Point {
    let (sin, cos) = bounded_sin_cos(parameter);
    let (rotation_sin, rotation_cos) = direction(angle);
    [
        center[0] + rx * cos * rotation_cos - ry * sin * rotation_sin,
        center[1] + rx * cos * rotation_sin + ry * sin * rotation_cos,
    ]
}
struct Ellipse {
    center: Point,
    rx: f64,
    ry: f64,
    angle: f64,
    start: f64,
    sweep: f64,
    close: bool,
}
fn ellipse_path(ellipse: Ellipse, path: &str, scale: f64) -> Result<Vec<Segment>, SceneError> {
    let Ellipse {
        center,
        rx,
        ry,
        angle,
        start,
        sweep,
        close,
    } = ellipse;
    // Linear interpolation is bounded by max(rx,ry)*delta^2/8. The 0.085px
    // target leaves 0.165px for arithmetic: at |x|<=pi the 20-term Taylor
    // remainder is below 1e-27, while the 2048-epsilon margin covers term
    // evaluation, argument reduction, interpolation and rotated coordinates.
    // Subnormal absolute error is covered by adding MIN_POSITIVE before scaling.
    // Reserve 0.125px for that margin and 0.025px for subdivision rounding;
    // the remaining 0.015px accommodates the final map guard.
    let radius = rx.max(ry);
    let magnitude = center[0].abs().max(center[1].abs()).max(radius);
    let rounding = (magnitude * f64::EPSILON * 2048.0 + f64::MIN_POSITIVE) * scale;
    let scaled_radius = radius * scale;
    if !rounding.is_finite() || rounding >= 0.125 || !scaled_radius.is_finite() {
        return Err(error(path, "", "ellipse accuracy cannot be represented"));
    }
    let sweep_rad = sweep.to_radians();
    let max_delta = (0.68 / scaled_radius)
        .sqrt()
        .min(std::f64::consts::FRAC_PI_2);
    let count_float = (sweep_rad.abs() / max_delta).ceil().max(1.0);
    if !count_float.is_finite()
        || count_float >= (isize::MAX as f64 / std::mem::size_of::<Segment>() as f64)
    {
        return Err(error(
            path,
            "",
            "ellipse subdivision count is not representable",
        ));
    }
    let count = count_float as usize;
    let capacity = count
        .checked_add(2)
        .ok_or_else(|| error(path, "", "ellipse subdivision overflow"))?;
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(capacity)
        .map_err(|_| error(path, "", "ellipse allocation failed"))?;
    let first = start.rem_euclid(360.0).to_radians();
    for index in 0..=count {
        let parameter = first + sweep_rad * (index as f64 / count as f64);
        let point = checked_point(ellipse_point(center, rx, ry, angle, parameter), path, "")?;
        if index == 0 {
            segments.push(Segment::Move(point));
        } else {
            segments.push(Segment::Line(point));
        }
    }
    if close {
        segments.push(Segment::Close);
    }
    Ok(segments)
}
fn include(bounds: &mut Bounds, p: Point) {
    for (axis, value) in p.into_iter().enumerate() {
        bounds.min[axis] = bounds.min[axis].min(value);
        bounds.max[axis] = bounds.max[axis].max(value);
    }
}
fn quadratic(a: f64, b: f64, c: f64, t: f64) -> f64 {
    let ab = a * (1.0 - t) + b * t;
    let bc = b * (1.0 - t) + c * t;
    ab * (1.0 - t) + bc * t
}
fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let ab = a * (1.0 - t) + b * t;
    let bc = b * (1.0 - t) + c * t;
    let cd = c * (1.0 - t) + d * t;
    quadratic(ab, bc, cd, t)
}
fn quadratic_bounds(bounds: &mut Bounds, a: Point, b: Point, c: Point) {
    include(bounds, c);
    for axis in 0..2 {
        let scale = a[axis].abs().max(b[axis].abs()).max(c[axis].abs());
        if scale == 0.0 {
            continue;
        }
        let (a1, b1, c1) = (a[axis] / scale, b[axis] / scale, c[axis] / scale);
        let denominator = a1 - 2.0 * b1 + c1;
        let t = (a1 - b1) / denominator;
        if t > 0.0 && t < 1.0 {
            let value = quadratic(a[axis], b[axis], c[axis], t);
            bounds.min[axis] = bounds.min[axis].min(value);
            bounds.max[axis] = bounds.max[axis].max(value);
        }
    }
}
fn cubic_bounds(bounds: &mut Bounds, a: Point, b: Point, c: Point, d: Point) {
    include(bounds, d);
    for axis in 0..2 {
        let scale = a[axis]
            .abs()
            .max(b[axis].abs())
            .max(c[axis].abs())
            .max(d[axis].abs());
        if scale == 0.0 {
            continue;
        }
        let (a1, b1, c1, d1) = (
            a[axis] / scale,
            b[axis] / scale,
            c[axis] / scale,
            d[axis] / scale,
        );
        let p = -a1 + 3.0 * b1 - 3.0 * c1 + d1;
        let q = 2.0 * (a1 - 2.0 * b1 + c1);
        let r = b1 - a1;
        let discriminant = q * q - 4.0 * p * r;
        let roots = if p == 0.0 {
            [-r / q, -1.0]
        } else if discriminant >= 0.0 {
            // The usual +/- formula loses the small root when q and sqrt(D) nearly cancel.
            let stable = -0.5 * (q + discriminant.sqrt().copysign(q));
            if stable == 0.0 {
                [0.0, -1.0]
            } else {
                [stable / p, r / stable]
            }
        } else {
            [-1.0, -1.0]
        };
        for t in roots {
            if t > 0.0 && t < 1.0 {
                let value = cubic(a[axis], b[axis], c[axis], d[axis], t);
                bounds.min[axis] = bounds.min[axis].min(value);
                bounds.max[axis] = bounds.max[axis].max(value);
            }
        }
    }
}
fn calculate_bounds(segments: &[Segment], path: &str) -> Result<Bounds, SceneError> {
    let mut bounds = Bounds {
        min: [f64::INFINITY; 2],
        max: [f64::NEG_INFINITY; 2],
    };
    let mut current = [0.0, 0.0];
    let mut first = current;
    for segment in segments {
        match *segment {
            Segment::Move(point) => {
                current = point;
                first = point;
                include(&mut bounds, point);
            }
            Segment::Line(point) => {
                include(&mut bounds, point);
                current = point;
            }
            Segment::Quadratic(control, point) => {
                quadratic_bounds(&mut bounds, current, control, point);
                current = point;
            }
            Segment::Cubic(a, b, point) => {
                cubic_bounds(&mut bounds, current, a, b, point);
                current = point;
            }
            Segment::Close => {
                include(&mut bounds, first);
                current = first;
            }
        }
    }
    if bounds
        .min
        .iter()
        .chain(bounds.max.iter())
        .any(|value| !value.is_finite())
    {
        return Err(error(path, "", "computed bounds are not finite"));
    }
    Ok(bounds)
}

#[cfg(test)]
mod tests;
