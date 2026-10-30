use super::*;
use crate::scene::{ErrorKind, Primitive, Shape, parse_scene};
fn load(d: &str, fill: &str) -> Result<crate::scene::Scene, crate::scene::SceneError> {
    let node = serde_json::json!({"schema_version":1,"shapes":[{"type":"path","d":d,"fill":fill}]});
    parse_scene(node.to_string().as_bytes())
}
#[test]
fn path_data_contract() {
    for path in [
        "M100-200L0.6.5",
        "M 1e2,-2E2 L+.6,.5 Q1 2 3 4 C5 6 7 8 9 10 Z",
        "M0 0L1 1M2 2L3 3Z M4 4Q5 6 7 8",
        "M0 0L1 1Z M2 2L3 3Z",
    ] {
        assert!(load(path, "none").is_ok(), "{path}");
    }
    let scene = load("M100-200L0.6.5Q1 2 3 4C5 6 7 8 9 10Z", "none").unwrap();
    let Shape::Primitive {
        primitive: primitive @ Primitive::Path { .. },
        style,
    } = &scene.shapes[0].shape
    else {
        panic!()
    };
    assert_eq!(
        crate::geometry::build_primitive(primitive, style, "shapes[0]", None, 10.24)
            .unwrap()
            .segments,
        [
            Segment::Move([100.0, -200.0]),
            Segment::Line([0.6, 0.5]),
            Segment::Quadratic([1.0, 2.0], [3.0, 4.0]),
            Segment::Cubic([5.0, 6.0], [7.0, 8.0], [9.0, 10.0]),
            Segment::Close
        ]
    );
    for path in [
        "",
        "L0 0",
        "m0 0L1 1",
        "M0 0A1 2",
        "M0 0L1 1 2 2",
        "<svg>",
        "M0 0",
        "M0 0Z",
        "M0 0L1",
        "M0 0L1 1e+",
        "M0 0L1e400 1",
        "M0 0L1 1,",
        "M0 0L1,,1",
        "M0 0L1 1ZL2 2",
        "M0 0L1 1!",
        "M,0 0L1 1",
        "M0 0Q1 2 3",
        "M0 0C1 2 3 4 5",
    ] {
        let error = load(path, "none").unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidPath, "{path}: {error}");
    }
}
#[test]
fn path_data_errors() {
    for (path, offset) in [
        ("M 0 0 A 1 2", 6),
        ("M 0 0 L 1e+ 2", 8),
        ("M0 0L1", 6),
        ("M0 0L1 1,", 8),
        ("M0 0L1 1é", 8),
        ("", 0),
        ("M0 0L1 1ZL2 2", 9),
    ] {
        let error = load(path, "none").unwrap_err();
        assert_eq!(
            (error.kind, error.path.as_str(), error.offset),
            (ErrorKind::InvalidPath, "shapes[0].d", Some(offset)),
            "{path}"
        );
        assert!(error.to_string().starts_with(&format!(
            "ERROR INVALID_PATH path=shapes[0].d offset={offset}: "
        )));
        assert!(path.is_char_boundary(offset));
    }
    let json = br#"{"schema_version":1,"shapes":[{"type":"group","shapes":[{"type":"path","d":"M\u0030 0 A 1 2"}]}]}"#;
    let error = parse_scene(json).unwrap_err();
    assert_eq!(
        (error.kind, error.path.as_str(), error.offset),
        (ErrorKind::InvalidPath, "shapes[0].shapes[0].d", Some(5))
    );
}
#[test]
fn path_data_fill() {
    assert!(load("M0 0L1 1Z", "white").is_ok());
    assert!(load("M0 0L1 1", "none").is_ok());
    for fill in ["white", "black"] {
        let error = load("M0 0L1 1Z M2 2L3 3", fill).unwrap_err();
        assert_eq!(
            (error.kind, error.path.as_str()),
            (ErrorKind::Shape, "shapes[0].fill")
        );
    }
    let json=br#"{"schema_version":1,"shapes":[{"type":"curve","from":[0,0],"ctrl1":[0,1],"ctrl2":[1,1],"to":[1,0],"fill":"white"}]}"#;
    let error = parse_scene(json).unwrap_err();
    assert_eq!(
        (error.kind, error.path.as_str()),
        (ErrorKind::Shape, "shapes[0].fill")
    );
}
