use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub const MAX_INPUT_BYTES: usize = 4_194_304;
pub const MAX_PATH_BYTES: usize = 65_536;
pub const MAX_PRIMITIVES: usize = 10_000;
pub type Point = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Structure,
    Shape,
    Limit,
    InvalidPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneError {
    pub kind: ErrorKind,
    pub path: String,
    pub detail: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub previous_path: Option<Box<str>>,
    pub offset: Option<usize>,
}

impl SceneError {
    pub(crate) fn new(kind: ErrorKind, path: &str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
            detail: detail.into(),
            line: None,
            column: None,
            previous_path: None,
            offset: None,
        }
    }
}
impl fmt::Display for SceneError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == ErrorKind::InvalidPath {
            write!(
                output,
                "ERROR INVALID_PATH path={} offset={}: {}",
                self.path,
                self.offset.unwrap_or(0),
                self.detail
            )?;
        } else if self.kind == ErrorKind::Shape {
            write!(
                output,
                "ERROR INVALID_SHAPE path={}: {}",
                self.path, self.detail
            )?;
        } else {
            write!(output, "ERROR path={}: {}", self.path, self.detail)?;
        }
        if let Some(previous) = &self.previous_path {
            write!(output, " (previously at {previous})")?;
        }
        if self.kind != ErrorKind::Syntax
            && let (Some(line), Some(column)) = (self.line, self.column)
        {
            write!(output, " (line {line}, column {column})")?;
        }
        Ok(())
    }
}
impl std::error::Error for SceneError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canvas {
    Landscape,
    Portrait,
    Square,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Thick,
    Thin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    None,
    White,
    Black,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipSide {
    Left,
    Top,
    Right,
    Bottom,
}
#[derive(Debug, PartialEq)]
pub struct Scene {
    pub schema_version: u8,
    pub canvas: Canvas,
    pub shapes: Vec<Node>,
}
#[derive(Debug, PartialEq)]
pub struct Node {
    pub id: Option<String>,
    pub allow_clip: Vec<ClipSide>,
    pub shape: Shape,
}
#[derive(Debug, PartialEq)]
pub struct Style {
    pub width: Width,
    pub fill: Fill,
}
#[derive(Debug, PartialEq)]
pub enum Line {
    Endpoints {
        from: Point,
        to: Point,
    },
    Polar {
        from: Point,
        angle: f64,
        length: f64,
    },
}
#[derive(Debug, PartialEq)]
pub enum Primitive {
    Line(Line),
    Polyline {
        pts: Vec<Point>,
    },
    Polygon {
        pts: Vec<Point>,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        angle: f64,
    },
    Arc {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        angle: f64,
        start: f64,
        sweep: f64,
        chord: bool,
    },
    Curve {
        from: Point,
        ctrl1: Point,
        ctrl2: Point,
        to: Point,
    },
    Path {
        d: String,
    },
}
#[derive(Debug, PartialEq)]
pub enum Shape {
    Primitive {
        style: Style,
        primitive: Primitive,
    },
    Group {
        at: Point,
        rotate: f64,
        scale: f64,
        flip_x: bool,
        shapes: Vec<Node>,
    },
    RepeatRotate {
        center: Point,
        count: u16,
        start: f64,
        step: Option<f64>,
        shapes: Vec<Node>,
    },
}

// This tree exists only between the duplicate-aware JSON parse and consuming conversion.
#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}
#[derive(Default)]
struct DecodeContext {
    path: String,
    open: Vec<usize>,
    failure: Option<SceneError>,
}
struct CheckedSeed<'a> {
    context: &'a mut DecodeContext,
}
impl DecodeContext {
    fn check_key<E: de::Error>(
        &mut self,
        fields: &BTreeMap<String, Json>,
        key: &str,
        path: &str,
    ) -> Result<(), E> {
        if fields.contains_key(key) {
            self.failure = Some(SceneError::new(
                ErrorKind::Structure,
                path,
                "duplicate JSON key",
            ));
            return Err(E::custom("duplicate JSON key"));
        }
        Ok(())
    }
}

pub(crate) fn field_path(parent: &str, field: &str) -> String {
    if parent == "$" {
        return field.into();
    }
    format!("{parent}.{field}")
}

fn key_path(parent: &str, field: &str) -> String {
    let simple = !field.is_empty()
        && field
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if !simple {
        let quoted = serde_json::to_string(field).expect("serializing a string cannot fail");
        return format!("{parent}[{quoted}]");
    }
    field_path(parent, field)
}

impl<'de> DeserializeSeed<'de> for CheckedSeed<'_> {
    type Value = Json;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for CheckedSeed<'_> {
    type Value = Json;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Json, E> {
        self.visit_f64(value as f64)
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Json, E> {
        self.visit_f64(value as f64)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        if !value.is_finite() {
            self.context.failure = Some(SceneError::new(
                ErrorKind::Structure,
                &self.context.path,
                "expected a finite number",
            ));
            return Err(E::custom("non-finite number"));
        }
        Ok(Json::Number(value))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Json, A::Error> {
        self.context.open.push(self.context.path.len());
        let mut values = Vec::new();
        loop {
            let length = self.context.path.len();
            self.context.path.push_str(&format!("[{}]", values.len()));
            let value = sequence.next_element_seed(CheckedSeed {
                context: self.context,
            })?;
            self.context.path.truncate(length);
            let Some(value) = value else {
                break;
            };
            values.push(value);
        }
        self.context.open.pop();
        Ok(Json::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        self.context.open.push(self.context.path.len());
        let mut fields = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            let length = self.context.path.len();
            let mut path = key_path(&self.context.path, &key);
            if self.context.path == "$" && !path.starts_with('$') {
                path = format!("$.{path}");
            }
            let display_path = path.strip_prefix("$.").unwrap_or(&path);
            self.context
                .check_key::<A::Error>(&fields, &key, display_path)?;
            self.context.path = path;
            let value = map.next_value_seed(CheckedSeed {
                context: self.context,
            })?;
            self.context.path.truncate(length);
            fields.insert(key, value);
        }
        self.context.open.pop();
        Ok(Json::Object(fields))
    }
}

pub fn parse_scene(input: &[u8]) -> Result<Scene, SceneError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(SceneError::new(
            ErrorKind::Limit,
            "$",
            "input exceeds 4194304 bytes",
        ));
    }
    let mut context = DecodeContext {
        path: "$".into(),
        ..DecodeContext::default()
    };
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let decoded = CheckedSeed {
        context: &mut context,
    }
    .deserialize(&mut deserializer);
    let tree = decoded.map_err(|error| decode_error(error, &mut context))?;
    deserializer
        .end()
        .map_err(|error| decode_error(error, &mut context))?;
    let mut root = Fields::new(tree, "$")?;
    let version = root.number("schema_version")?;
    if version != 1.0 {
        return Err(root.error("schema_version", "expected schema version 1"));
    }
    let canvas = match root
        .optional_string("canvas")?
        .as_deref()
        .unwrap_or("landscape")
    {
        "landscape" => Canvas::Landscape,
        "portrait" => Canvas::Portrait,
        "square" => Canvas::Square,
        _ => return Err(root.error("canvas", "unknown canvas")),
    };
    let shapes = decode_nodes(root.take("shapes")?, "shapes", Nesting::Root)?;
    root.finish()?;
    let scene = Scene {
        schema_version: 1,
        canvas,
        shapes,
    };
    crate::validate::validate_scene(&scene)?;
    Ok(scene)
}
fn decode_error(error: serde_json::Error, context: &mut DecodeContext) -> SceneError {
    let path = context
        .open
        .last()
        .map(|&length| &context.path[..length])
        .unwrap_or("$");
    let path = path.strip_prefix("$.").unwrap_or(path);
    let mut failure = context
        .failure
        .take()
        .unwrap_or_else(|| SceneError::new(ErrorKind::Syntax, path, error.to_string()));
    failure.line = Some(error.line());
    failure.column = Some(error.column());
    failure
}

struct Fields {
    values: BTreeMap<String, Json>,
    path: String,
}
impl Fields {
    fn new(value: Json, path: &str) -> Result<Self, SceneError> {
        let Json::Object(values) = value else {
            return Err(SceneError::new(
                ErrorKind::Structure,
                path,
                "expected an object",
            ));
        };
        Ok(Self {
            values,
            path: path.into(),
        })
    }
    fn error(&self, field: &str, detail: &str) -> SceneError {
        SceneError::new(ErrorKind::Structure, &field_path(&self.path, field), detail)
    }
    fn take(&mut self, field: &str) -> Result<Json, SceneError> {
        self.values
            .remove(field)
            .ok_or_else(|| self.error(field, "missing required field"))
    }
    fn optional<T>(
        &mut self,
        field: &str,
        decode: fn(Json, &str) -> Result<T, SceneError>,
    ) -> Result<Option<T>, SceneError> {
        let Some(value) = self.values.remove(field) else {
            return Ok(None);
        };
        decode(value, &field_path(&self.path, field)).map(Some)
    }
    fn number(&mut self, field: &str) -> Result<f64, SceneError> {
        let value = self.take(field)?;
        decode_number(value, &field_path(&self.path, field))
    }
    fn number_or(&mut self, field: &str, default: f64) -> Result<f64, SceneError> {
        Ok(self.optional(field, decode_number)?.unwrap_or(default))
    }
    fn point(&mut self, field: &str) -> Result<Point, SceneError> {
        let value = self.take(field)?;
        decode_point(value, &field_path(&self.path, field))
    }
    fn boolean_or(&mut self, field: &str, default: bool) -> Result<bool, SceneError> {
        Ok(self.optional(field, decode_boolean)?.unwrap_or(default))
    }
    fn string(&mut self, field: &str) -> Result<String, SceneError> {
        let value = self.take(field)?;
        decode_string(value, &field_path(&self.path, field))
    }
    fn optional_string(&mut self, field: &str) -> Result<Option<String>, SceneError> {
        self.optional(field, decode_string)
    }
    fn finish(self) -> Result<(), SceneError> {
        if let Some((key, _)) = self.values.first_key_value() {
            return Err(SceneError::new(
                ErrorKind::Structure,
                &key_path(&self.path, key),
                "unknown field",
            ));
        }
        Ok(())
    }
}
fn decode_number(value: Json, path: &str) -> Result<f64, SceneError> {
    let Json::Number(number) = value else {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "expected a finite number",
        ));
    };
    Ok(number)
}
fn decode_boolean(value: Json, path: &str) -> Result<bool, SceneError> {
    let Json::Bool(boolean) = value else {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "expected a boolean",
        ));
    };
    Ok(boolean)
}
fn decode_string(value: Json, path: &str) -> Result<String, SceneError> {
    let Json::String(string) = value else {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "expected a string",
        ));
    };
    Ok(string)
}
fn decode_array(value: Json, path: &str) -> Result<Vec<Json>, SceneError> {
    let Json::Array(array) = value else {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "expected an array",
        ));
    };
    Ok(array)
}
fn decode_point(value: Json, path: &str) -> Result<Point, SceneError> {
    let values = decode_array(value, path)?;
    let [x, y]: [Json; 2] = values.try_into().map_err(|_| {
        SceneError::new(
            ErrorKind::Structure,
            path,
            "expected exactly two coordinates",
        )
    })?;
    Ok([
        decode_number(x, &format!("{path}[0]"))?,
        decode_number(y, &format!("{path}[1]"))?,
    ])
}
fn decode_points(value: Json, path: &str) -> Result<Vec<Point>, SceneError> {
    let values = decode_array(value, path)?;
    let mut points = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        points.push(decode_point(value, &format!("{path}[{index}]"))?);
    }
    Ok(points)
}
#[derive(Clone, Copy)]
enum Nesting {
    Root,
    Group,
    Repeat,
}
fn decode_nodes(value: Json, path: &str, nesting: Nesting) -> Result<Vec<Node>, SceneError> {
    let values = decode_array(value, path)?;
    if values.is_empty() {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "expected nonempty shapes",
        ));
    }
    if values.len() > MAX_PRIMITIVES {
        return Err(SceneError::new(
            ErrorKind::Limit,
            path,
            "shapes exceeds 10000 entries",
        ));
    }
    let mut nodes = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        nodes.push(decode_node(value, &format!("{path}[{index}]"), nesting)?);
    }
    Ok(nodes)
}
fn decode_clip(value: Json, path: &str) -> Result<Vec<ClipSide>, SceneError> {
    let values = decode_array(value, path)?;
    if values.len() > 4 {
        return Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "at most four clipping sides",
        ));
    }
    let mut sides = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        let side_path = format!("{path}[{index}]");
        let side = decode_clip_side(value, &side_path)?;
        sides.push(side);
    }
    Ok(sides)
}
fn decode_clip_side(value: Json, path: &str) -> Result<ClipSide, SceneError> {
    match decode_string(value, path)?.as_str() {
        "left" => Ok(ClipSide::Left),
        "top" => Ok(ClipSide::Top),
        "right" => Ok(ClipSide::Right),
        "bottom" => Ok(ClipSide::Bottom),
        _ => Err(SceneError::new(
            ErrorKind::Structure,
            path,
            "unknown clipping side",
        )),
    }
}
fn decode_node(value: Json, path: &str, nesting: Nesting) -> Result<Node, SceneError> {
    let mut fields = Fields::new(value, path)?;
    let shape_type = fields.string("type")?;
    let id = fields.optional_string("id")?;
    let allow_clip = fields
        .optional("allow_clip", decode_clip)?
        .unwrap_or_default();
    let shape = match shape_type.as_str() {
        "group" if matches!(nesting, Nesting::Root) => decode_group(&mut fields)?,
        "repeat_rotate" if !matches!(nesting, Nesting::Repeat) => decode_repeat(&mut fields)?,
        "group" | "repeat_rotate" => {
            return Err(fields.error("type", "container is not allowed at this nesting level"));
        }
        _ => decode_primitive(&shape_type, &mut fields)?,
    };
    fields.finish()?;
    Ok(Node {
        id,
        allow_clip,
        shape,
    })
}
fn decode_group(fields: &mut Fields) -> Result<Shape, SceneError> {
    let at = fields.optional("at", decode_point)?.unwrap_or([0.0, 0.0]);
    let rotate = fields.number_or("rotate", 0.0)?;
    let scale = fields.number_or("scale", 1.0)?;
    let flip_x = fields.boolean_or("flip_x", false)?;
    let shapes = decode_nodes(
        fields.take("shapes")?,
        &field_path(&fields.path, "shapes"),
        Nesting::Group,
    )?;
    Ok(Shape::Group {
        at,
        rotate,
        scale,
        flip_x,
        shapes,
    })
}
fn decode_repeat(fields: &mut Fields) -> Result<Shape, SceneError> {
    let center = fields.point("center")?;
    let count = fields.number("count")?;
    if !(1.0..=256.0).contains(&count) || count.fract() != 0.0 {
        return Err(fields.error("count", "expected an integer in 1..256"));
    }
    let start = fields.number_or("start", 0.0)?;
    let step = fields.optional("step", decode_number)?;
    let shapes = decode_nodes(
        fields.take("shapes")?,
        &field_path(&fields.path, "shapes"),
        Nesting::Repeat,
    )?;
    Ok(Shape::RepeatRotate {
        center,
        count: count as u16,
        start,
        step,
        shapes,
    })
}
fn decode_primitive(shape_type: &str, fields: &mut Fields) -> Result<Shape, SceneError> {
    let width = match fields
        .optional_string("width")?
        .as_deref()
        .unwrap_or("thick")
    {
        "thick" => Width::Thick,
        "thin" => Width::Thin,
        _ => return Err(fields.error("width", "expected thick or thin")),
    };
    let fill = match fields.optional_string("fill")?.as_deref().unwrap_or("none") {
        "none" => Fill::None,
        "white" => Fill::White,
        "black" => Fill::Black,
        _ => return Err(fields.error("fill", "expected none, white or black")),
    };
    let primitive = match shape_type {
        "line" => Primitive::Line(decode_line(fields)?),
        "polyline" => Primitive::Polyline {
            pts: decode_points(fields.take("pts")?, &field_path(&fields.path, "pts"))?,
        },
        "polygon" => Primitive::Polygon {
            pts: decode_points(fields.take("pts")?, &field_path(&fields.path, "pts"))?,
        },
        "rect" => Primitive::Rect {
            x: fields.number("x")?,
            y: fields.number("y")?,
            w: fields.number("w")?,
            h: fields.number("h")?,
        },
        "circle" => Primitive::Circle {
            cx: fields.number("cx")?,
            cy: fields.number("cy")?,
            r: fields.number("r")?,
        },
        "ellipse" => Primitive::Ellipse {
            cx: fields.number("cx")?,
            cy: fields.number("cy")?,
            rx: fields.number("rx")?,
            ry: fields.number("ry")?,
            angle: fields.number_or("angle", 0.0)?,
        },
        "arc" => Primitive::Arc {
            cx: fields.number("cx")?,
            cy: fields.number("cy")?,
            rx: fields.number("rx")?,
            ry: fields.number("ry")?,
            angle: fields.number_or("angle", 0.0)?,
            start: fields.number("start")?,
            sweep: fields.number("sweep")?,
            chord: fields.boolean_or("chord", false)?,
        },
        "curve" => Primitive::Curve {
            from: fields.point("from")?,
            ctrl1: fields.point("ctrl1")?,
            ctrl2: fields.point("ctrl2")?,
            to: fields.point("to")?,
        },
        "path" => Primitive::Path {
            d: fields.string("d")?,
        },
        _ => return Err(fields.error("type", "unknown shape type")),
    };
    Ok(Shape::Primitive {
        style: Style { width, fill },
        primitive,
    })
}
fn decode_line(fields: &mut Fields) -> Result<Line, SceneError> {
    let from = fields.point("from")?;
    let has_to = fields.values.contains_key("to");
    if has_to && fields.values.contains_key("angle") {
        return Err(fields.error("angle", "cannot combine endpoint and polar line forms"));
    }
    if has_to && fields.values.contains_key("length") {
        return Err(fields.error("length", "cannot combine endpoint and polar line forms"));
    }
    if has_to {
        return Ok(Line::Endpoints {
            from,
            to: fields.point("to")?,
        });
    }
    Ok(Line::Polar {
        from,
        angle: fields.number("angle")?,
        length: fields.number("length")?,
    })
}

#[cfg(test)]
mod tests;
