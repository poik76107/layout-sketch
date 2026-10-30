use std::collections::HashMap;

use crate::scene::{
    ErrorKind, Fill, Line, MAX_PATH_BYTES, MAX_PRIMITIVES, Node, Point, Primitive, Scene,
    SceneError, Shape, field_path,
};

pub(crate) fn validate_scene(scene: &Scene) -> Result<(), SceneError> {
    let mut ids = HashMap::new();
    validate_nodes(&scene.shapes, "shapes", &mut ids)?;
    Ok(())
}
fn shape_error(path: &str, field: &str, detail: &str) -> SceneError {
    SceneError::new(ErrorKind::Shape, &field_path(path, field), detail)
}
fn validate_nodes(
    nodes: &[Node],
    path: &str,
    ids: &mut HashMap<String, String>,
) -> Result<usize, SceneError> {
    let mut total = 0usize;
    for (index, node) in nodes.iter().enumerate() {
        let node_path = format!("{path}[{index}]");
        validate_metadata(node, &node_path, ids)?;
        let count = validate_node(node, &node_path, ids)?;
        total = total.checked_add(count).ok_or_else(|| {
            SceneError::new(ErrorKind::Limit, path, "expanded primitive count overflow")
        })?;
        check_primitive_limit(total, path)?;
    }
    Ok(total)
}
fn validate_node(
    node: &Node,
    path: &str,
    ids: &mut HashMap<String, String>,
) -> Result<usize, SceneError> {
    match &node.shape {
        Shape::Primitive { style, primitive } => {
            validate_primitive(primitive, path)?;
            validate_fill(primitive, style.fill, path)?;
            crate::geometry::validate_finite(primitive, path)?;
            Ok(1)
        }
        Shape::Group { scale, shapes, .. } => {
            validate_positive(*scale, path, "scale")?;
            validate_nodes(shapes, &field_path(path, "shapes"), ids)
        }
        Shape::RepeatRotate { count, shapes, .. } => {
            let children = validate_nodes(shapes, &field_path(path, "shapes"), ids)?;
            let total = children.checked_mul(usize::from(*count)).ok_or_else(|| {
                SceneError::new(ErrorKind::Limit, path, "expanded primitive count overflow")
            })?;
            check_primitive_limit(total, path)?;
            Ok(total)
        }
    }
}
fn check_primitive_limit(total: usize, path: &str) -> Result<(), SceneError> {
    if total > MAX_PRIMITIVES {
        return Err(SceneError::new(
            ErrorKind::Limit,
            path,
            "expanded primitive count exceeds 10000",
        ));
    }
    Ok(())
}
fn validate_metadata(
    node: &Node,
    path: &str,
    ids: &mut HashMap<String, String>,
) -> Result<(), SceneError> {
    if let Some(id) = &node.id {
        validate_id(id, path, ids)?;
    }
    let duplicate = node
        .allow_clip
        .iter()
        .enumerate()
        .find(|(index, side)| node.allow_clip[..*index].contains(side));
    if let Some((index, _)) = duplicate {
        return Err(shape_error(
            path,
            &format!("allow_clip[{index}]"),
            "duplicate clipping side",
        ));
    }
    Ok(())
}
fn validate_id(id: &str, path: &str, ids: &mut HashMap<String, String>) -> Result<(), SceneError> {
    let bytes = id.as_bytes();
    let has_valid_first = bytes.first().is_some_and(u8::is_ascii_alphabetic);
    let has_valid_rest = bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-');
    if !has_valid_first || !has_valid_rest || bytes.len() > 64 {
        return Err(shape_error(
            path,
            "id",
            "expected 1..64 ASCII characters, first a letter, then letters, digits, _ or -",
        ));
    }
    let id_path = field_path(path, "id");
    if let Some(previous) = ids.get(id) {
        let mut error = shape_error(path, "id", "duplicate source id");
        error.previous_path = Some(previous.clone().into_boxed_str());
        return Err(error);
    }
    ids.insert(id.into(), id_path);
    Ok(())
}
fn validate_positive(value: f64, path: &str, field: &str) -> Result<(), SceneError> {
    if value <= 0.0 {
        return Err(shape_error(path, field, "expected a positive number"));
    }
    Ok(())
}
fn validate_fill(primitive: &Primitive, fill: Fill, path: &str) -> Result<(), SceneError> {
    if let Primitive::Path { d } = primitive {
        let parsed = crate::path_data::parse_path(d).map_err(|error| path_error(path, error))?;
        if fill != Fill::None && !parsed.all_closed {
            return Err(shape_error(
                path,
                "fill",
                "filled path requires closed subcontours",
            ));
        }
    }
    let is_open = matches!(
        primitive,
        Primitive::Line(_)
            | Primitive::Polyline { .. }
            | Primitive::Curve { .. }
            | Primitive::Arc { chord: false, .. }
    );
    if is_open && fill != Fill::None {
        return Err(shape_error(
            path,
            "fill",
            "open primitive requires fill none",
        ));
    }
    Ok(())
}
fn validate_primitive(primitive: &Primitive, path: &str) -> Result<(), SceneError> {
    match primitive {
        Primitive::Line(Line::Endpoints { from, to }) if from == to => {
            Err(shape_error(path, "to", "line endpoints must differ"))
        }
        Primitive::Line(Line::Polar { length, .. }) => validate_positive(*length, path, "length"),
        Primitive::Polyline { pts } => validate_polyline(pts, path),
        Primitive::Polygon { pts } => validate_polygon(pts, path),
        Primitive::Rect { w, h, .. } => {
            validate_positive(*w, path, "w")?;
            validate_positive(*h, path, "h")
        }
        Primitive::Circle { r, .. } => validate_positive(*r, path, "r"),
        Primitive::Ellipse { rx, ry, .. } => validate_radii(*rx, *ry, path),
        Primitive::Arc { rx, ry, sweep, .. } => validate_arc(*rx, *ry, *sweep, path),
        Primitive::Path { d } => validate_path_string(d, path),
        _ => Ok(()),
    }
}
fn validate_radii(rx: f64, ry: f64, path: &str) -> Result<(), SceneError> {
    validate_positive(rx, path, "rx")?;
    validate_positive(ry, path, "ry")
}
fn validate_arc(rx: f64, ry: f64, sweep: f64, path: &str) -> Result<(), SceneError> {
    validate_radii(rx, ry, path)?;
    if sweep == 0.0 || sweep.abs() > 360.0 {
        return Err(shape_error(path, "sweep", "expected 0 < abs(sweep) <= 360"));
    }
    Ok(())
}
fn validate_path_string(d: &str, path: &str) -> Result<(), SceneError> {
    if d.len() > MAX_PATH_BYTES {
        return Err(SceneError::new(
            ErrorKind::Limit,
            &field_path(path, "d"),
            "decoded path exceeds 65536 UTF-8 bytes",
        ));
    }
    crate::path_data::parse_path(d).map_err(|error| path_error(path, error))?;
    Ok(())
}
fn path_error(path: &str, error: crate::path_data::PathError) -> SceneError {
    let mut result = SceneError::new(ErrorKind::InvalidPath, &field_path(path, "d"), error.detail);
    result.offset = Some(error.offset);
    result
}
fn validate_adjacent_points(points: &[Point], path: &str) -> Result<(), SceneError> {
    let duplicate = points.windows(2).position(|pair| pair[0] == pair[1]);
    if let Some(index) = duplicate {
        return Err(shape_error(
            path,
            &format!("pts[{}]", index + 1),
            "consecutive points must differ",
        ));
    }
    Ok(())
}
fn validate_polyline(points: &[Point], path: &str) -> Result<(), SceneError> {
    if points.len() < 2 {
        return Err(shape_error(path, "pts", "expected at least two points"));
    }
    validate_adjacent_points(points, path)
}
fn effective_polygon_points(points: &[Point]) -> &[Point] {
    if points.len() > 1 && points.first() == points.last() {
        return &points[..points.len() - 1];
    }
    points
}
fn validate_polygon(source: &[Point], path: &str) -> Result<(), SceneError> {
    let points = effective_polygon_points(source);
    if points.len() < 3 {
        return Err(shape_error(
            path,
            "pts",
            "expected at least three effective polygon points",
        ));
    }
    validate_adjacent_points(points, path)?;
    if points.first() == points.last() {
        return Err(shape_error(
            path,
            &format!("pts[{}]", points.len() - 1),
            "cyclic consecutive points must differ",
        ));
    }
    if are_points_collinear(points) {
        return Err(shape_error(
            path,
            "pts",
            "polygon points are completely collinear",
        ));
    }
    Ok(())
}
fn are_points_collinear(points: &[Point]) -> bool {
    let Some(first) = points.first() else {
        return true;
    };
    let Some(baseline) = points.iter().find(|point| *point != first) else {
        return true;
    };
    points
        .iter()
        .all(|point| is_determinant_zero(*first, *baseline, *point))
}

// A finite binary64 is sign * significand * 2^exponent. Subnormals use -1074;
// normals have <=53 significand bits and exponents -1074..971.
fn decompose_coordinate(value: f64) -> (bool, u64, i32) {
    let bits = value.to_bits();
    let is_negative = bits >> 63 != 0;
    let fraction = bits & ((1u64 << 52) - 1);
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        return (is_negative, fraction, -1074);
    }
    (is_negative, fraction | (1u64 << 52), exponent - 1075)
}
fn is_determinant_zero(a: Point, b: Point, c: Point) -> bool {
    // Six exact products, without rounded coordinate subtraction. Products have
    // <=106 bits, exponents -2148..1942. Aligning at 2^-2148 spans 4090 bits;
    // summing six terms needs <=4200 bits including sign, below 66*64 = 4224.
    let mut accumulator = [0u64; 66];
    add_coordinate_product(&mut accumulator, a[0], b[1], false);
    add_coordinate_product(&mut accumulator, b[0], c[1], false);
    add_coordinate_product(&mut accumulator, c[0], a[1], false);
    add_coordinate_product(&mut accumulator, a[1], b[0], true);
    add_coordinate_product(&mut accumulator, b[1], c[0], true);
    add_coordinate_product(&mut accumulator, c[1], a[0], true);
    accumulator.iter().all(|word| *word == 0)
}
fn add_coordinate_product(accumulator: &mut [u64; 66], left: f64, right: f64, subtract: bool) {
    let (left_negative, left_significand, left_exponent) = decompose_coordinate(left);
    let (right_negative, right_significand, right_exponent) = decompose_coordinate(right);
    let product = u128::from(left_significand) * u128::from(right_significand);
    let shift = (left_exponent + right_exponent + 2148) as usize;
    let is_negative = left_negative ^ right_negative ^ subtract;
    let words = shift_product(product, shift % 64);
    // Signed two's-complement accumulation modulo 2^4224. Addition propagates
    // carry, subtraction propagates borrow through all higher words (including
    // zero words, so negative terms sign-extend). The final carry/borrow is
    // discarded. The bound above excludes a nonzero exact sum aliasing zero.
    let mut carry = false;
    for (offset, word) in accumulator.iter_mut().skip(shift / 64).enumerate() {
        let term = words.get(offset).copied().unwrap_or(0);
        let (value, first_carry) = combine_word(*word, term, is_negative);
        let (value, second_carry) = combine_word(value, u64::from(carry), is_negative);
        *word = value;
        carry = first_carry || second_carry;
    }
}
fn combine_word(left: u64, right: u64, subtract: bool) -> (u64, bool) {
    if subtract {
        return left.overflowing_sub(right);
    }
    left.overflowing_add(right)
}
fn shift_product(product: u128, shift: usize) -> [u64; 3] {
    let low = product as u64;
    let high = (product >> 64) as u64;
    if shift == 0 {
        return [low, high, 0];
    }
    [
        low << shift,
        (high << shift) | (low >> (64 - shift)),
        high >> (64 - shift),
    ]
}

#[cfg(test)]
mod tests;
