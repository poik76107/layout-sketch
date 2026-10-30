use super::*;
use crate::scene::{ClipSide, Style, Width, parse_scene};

fn load_node(node: &str) -> Result<Scene, SceneError> {
    parse_scene(format!(r#"{{"schema_version":1,"shapes":[{node}]}}"#).as_bytes())
}
fn load_polygon(points: &str) -> Result<Scene, SceneError> {
    load_node(&format!(r#"{{"type":"polygon","pts":[{points}]}}"#))
}
const CIRCLE: &str = r#"{"type":"circle","cx":0,"cy":0,"r":1}"#;

#[test]
fn scene_primitive_rules() {
    let scene = load_node(CIRCLE).unwrap();
    assert!(matches!(
        scene.shapes[0].shape,
        Shape::Primitive {
            style: Style {
                width: Width::Thick,
                fill: Fill::None
            },
            ..
        }
    ));
    let ellipse = load_node(r#"{"type":"ellipse","cx":0,"cy":0,"rx":1,"ry":2}"#).unwrap();
    assert!(matches!(
        ellipse.shapes[0].shape,
        Shape::Primitive {
            primitive: Primitive::Ellipse { angle: 0.0, .. },
            ..
        }
    ));
    let arc =
        load_node(r#"{"type":"arc","cx":0,"cy":0,"rx":1,"ry":2,"start":0,"sweep":90}"#).unwrap();
    assert!(matches!(
        arc.shapes[0].shape,
        Shape::Primitive {
            primitive: Primitive::Arc {
                angle: 0.0,
                chord: false,
                ..
            },
            ..
        }
    ));
    let valid = [
        r#"{"type":"line","from":[-1,0.5],"to":[1,2]}"#,
        r#"{"type":"line","from":[0,0],"angle":-720.5,"length":1}"#,
        r#"{"type":"polyline","pts":[[0,0],[1,1],[2,2],[0,0]]}"#,
        r#"{"type":"polygon","pts":[[0,0],[1,1],[0,1],[1,0]],"fill":"black"}"#,
        r#"{"type":"rect","x":-1,"y":0.5,"w":1,"h":2,"fill":"white"}"#,
        r#"{"type":"arc","cx":0,"cy":0,"rx":1,"ry":1,"start":-1000,"sweep":360,"chord":true,"fill":"black"}"#,
        r#"{"type":"arc","cx":0,"cy":0,"rx":1,"ry":1,"start":0,"sweep":-360}"#,
        r#"{"type":"path","d":"M0 0L1 1Z","fill":"white"}"#,
        r#"{"type":"curve","from":[0,0],"ctrl1":[0,1],"ctrl2":[1,1],"to":[1,0]}"#,
        r#"{"type":"polyline","pts":[[0,0],[1,1]]}"#,
    ];
    for node in valid {
        assert!(load_node(node).is_ok(), "{node}");
    }
    assert_eq!(
        load_node(r#"{"type":"path","d":"opaque stage-1 string"}"#)
            .unwrap_err()
            .kind,
        ErrorKind::InvalidPath
    );
    for node in [
        r#"{"type":"line","from":[0,0],"to":[0,0]}"#,
        r#"{"type":"line","from":[0,0],"length":1}"#,
        r#"{"type":"line","from":[0,0],"angle":0}"#,
        r#"{"type":"line","from":[0,0]}"#,
        r#"{"type":"line","from":[0,0],"to":[1,1],"angle":0}"#,
        r#"{"type":"line","from":[0,0],"to":[1,1],"length":1}"#,
        r#"{"type":"line","from":[0,0],"angle":0,"length":0}"#,
        r#"{"type":"polyline","pts":[[0,0]]}"#,
        r#"{"type":"polyline","pts":[[0,0],[0,0]]}"#,
        r#"{"type":"rect","x":0,"y":0,"w":1,"h":1,"angle":1}"#,
        r#"{"type":"path","d":""}"#,
        r#"{"type":"curve","from":[0,0],"ctrl1":[0,1],"to":[1,0]}"#,
    ] {
        assert!(load_node(node).is_err(), "{node}");
    }
    for value in ["0", "-1"] {
        for node in [
            format!(r#"{{"type":"circle","cx":0,"cy":0,"r":{value}}}"#),
            format!(r#"{{"type":"rect","x":0,"y":0,"w":{value},"h":1}}"#),
            format!(r#"{{"type":"rect","x":0,"y":0,"w":1,"h":{value}}}"#),
            format!(r#"{{"type":"ellipse","cx":0,"cy":0,"rx":{value},"ry":1}}"#),
            format!(r#"{{"type":"ellipse","cx":0,"cy":0,"rx":1,"ry":{value}}}"#),
            format!(r#"{{"type":"arc","cx":0,"cy":0,"rx":{value},"ry":1,"start":0,"sweep":1}}"#),
            format!(r#"{{"type":"arc","cx":0,"cy":0,"rx":1,"ry":{value},"start":0,"sweep":1}}"#),
            format!(r#"{{"type":"line","from":[0,0],"angle":0,"length":{value}}}"#),
        ] {
            assert!(load_node(&node).is_err(), "{node}");
        }
    }
    for sweep in ["0", "360.1", "-361"] {
        assert!(
            load_node(&format!(
                r#"{{"type":"arc","cx":0,"cy":0,"rx":1,"ry":1,"start":0,"sweep":{sweep}}}"#
            ))
            .is_err()
        );
    }
    for node in [valid[0], valid[1], valid[2], valid[6], valid[8]] {
        let mut value: serde_json::Value = serde_json::from_str(node).unwrap();
        value["fill"] = "white".into();
        let error = load_node(&value.to_string()).unwrap_err();
        assert_eq!(error.path, "shapes[0].fill");
    }
}
#[test]
fn scene_polygon_rules() {
    for points in [
        "[0,0],[1,0],[0,1]",
        "[0,0],[1,0],[0,1],[0,0]",
        "[0,0],[1,0],[0,0],[0,1]",
    ] {
        let scene = load_polygon(points).unwrap();
        let Shape::Primitive {
            primitive: Primitive::Polygon { pts },
            ..
        } = &scene.shapes[0].shape
        else {
            panic!()
        };
        let expected: Vec<Point> = serde_json::from_str(&format!("[{points}]")).unwrap();
        assert_eq!(*pts, expected);
    }
    for (points, path) in [
        ("[0,0],[1,0],[0,0]", "shapes[0].pts"),
        ("[0,0],[1,0],[0,1],[0,0],[0,0]", "shapes[0].pts[3]"),
        ("[0,0],[0,0],[1,0],[0,1],[0,0]", "shapes[0].pts[1]"),
        ("[0,0],[1,0],[0,1],[0,1],[0,0]", "shapes[0].pts[3]"),
        ("[0,0],[1,0],[2,0],[0,0]", "shapes[0].pts"),
    ] {
        let error = load_polygon(points).unwrap_err();
        assert_eq!((error.kind, error.path.as_str()), (ErrorKind::Shape, path));
    }
    // Signed zeros count as equal; the closing marker is removed only once.
    assert!(load_polygon("[-0,0],[1,0],[0,1],[0,-0]").is_ok());
    assert!(load_polygon("[-0,0],[0,-0],[1,0],[0,1]").is_err());
}
#[test]
fn scene_collinearity() {
    assert!(
        load_polygon("[-1.234375,2.8203125],[10.859375,16.0390625],[-16.3515625,-13.703125]")
            .is_err()
    );
    assert!(
        load_polygon(
            "[-1.234375,2.8203125],[10.859375,16.0390625],[-16.3515625,-13.703124999999998]"
        )
        .is_ok()
    );
    for (points, is_valid) in [
        ("[0,0],[1,1],[2,2]", false),
        ("[0,0],[1e-200,1e-200],[2e-200,2e-200]", false),
        ("[0,0],[1e-200,0],[0,1e-200]", true),
        ("[0,0],[5e-324,0],[0,5e-324]", true),
        ("[0,0],[5e-324,5e-324],[1e-323,1e-323]", false),
        ("[0.1,0.1],[0.2,0.2],[0.3,0.3]", false),
        ("[0.1,0.2],[0.2,0.3],[0.3,0.4]", true),
        (
            "[-1.7976931348623157e308,-1.7976931348623157e308],[0,0],[1.7976931348623157e308,1.7976931348623157e308]",
            false,
        ),
        (
            "[-1.7976931348623157e308,-1.7976931348623157e308],[1.7976931348623157e308,-1.7976931348623157e308],[0,1.7976931348623157e308]",
            false,
        ),
        (
            "[-1.7976931348623157e308,5e-324],[0,5e-324],[1.7976931348623157e308,5e-324]",
            false,
        ),
        (
            "[1.7976931348623157e308,1.7976931348623157e308],[-1.7976931348623157e308,-1.7976931348623157e308],[5e-324,0]",
            false,
        ),
        (
            "[1.7976931348623157e308,1.7976931348623157e308],[5e-324,5e-324],[0,0]",
            false,
        ),
        ("[-0,0],[1,1],[2,2]", false),
    ] {
        assert_eq!(load_polygon(points).is_ok(), is_valid, "{points}");
    }
    assert!(load_node(r#"{"type":"group","scale":1e200,"shapes":[{"type":"polygon","pts":[[0,0],[1e-200,0],[0,1e-200]]}]}"#).is_ok());
    // Both orientations, all coordinate roles and carry/borrow cancellation.
    let a = [f64::MAX, f64::from_bits(1)];
    let b = [-f64::MAX, f64::from_bits(1)];
    let c = [0.0, -f64::from_bits(1)];
    for points in [
        [a, b, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
        [a, c, b],
        [b, a, c],
    ] {
        assert!(!are_points_collinear(&points));
    }
    assert!(are_points_collinear(&[
        [1.0, 1.0],
        [1.0, 1.0],
        [2.0, 2.0],
        [3.0, 3.0]
    ]));
    assert!(!are_points_collinear(&[
        [0.0, 0.0],
        [1.0, 0.0],
        [0.0, 0.0],
        [0.0, 1.0]
    ]));
    assert!(is_determinant_zero(
        [f64::MAX, f64::MAX],
        [f64::MAX, f64::MAX],
        [f64::MAX, f64::MAX]
    ));
}
#[test]
fn scene_identity_and_style() {
    let error = load_node(r#"{"type":"group","id":"same","shapes":[{"type":"circle","id":"same","cx":0,"cy":0,"r":1}]}"#).unwrap_err();
    assert_eq!(error.previous_path.as_deref(), Some("shapes[0].id"));
    let scene = r#"{"schema_version":1,"shapes":[{"type":"group","shapes":[{"id":"same","type":"circle","cx":0,"cy":0,"r":1}]},{"type":"group","shapes":[{"id":"same","type":"circle","cx":0,"cy":0,"r":1}]}]}"#;
    let error = parse_scene(scene.as_bytes()).unwrap_err();
    assert_eq!(error.path, "shapes[1].shapes[0].id");
    assert_eq!(
        error.previous_path.as_deref(),
        Some("shapes[0].shapes[0].id")
    );
    for id in ["a".into(), "A0_-".into(), "a".repeat(64)] {
        assert!(
            load_node(&format!(
                r#"{{"type":"circle","cx":0,"cy":0,"r":1,"id":"{id}"}}"#
            ))
            .is_ok()
        );
    }
    for id in [
        "".into(),
        "a".repeat(65),
        "é".into(),
        "1a".into(),
        "a.b".into(),
        "a b".into(),
    ] {
        assert_eq!(
            load_node(&format!(
                r#"{{"type":"circle","cx":0,"cy":0,"r":1,"id":"{id}"}}"#
            ))
            .unwrap_err()
            .path,
            "shapes[0].id"
        );
    }
    let scene = load_node(r#"{"type":"repeat_rotate","id":"repeat","center":[0,0],"count":6,"shapes":[{"type":"circle","id":"child","cx":0,"cy":0,"r":1}]}"#).unwrap();
    assert!(matches!(
        scene.shapes[0].shape,
        Shape::RepeatRotate { count: 6, .. }
    ));
    let scene = load_node(r#"{"type":"circle","cx":0,"cy":0,"r":1,"width":"thin","fill":"black","allow_clip":["right","top","bottom","left"]}"#).unwrap();
    assert_eq!(
        scene.shapes[0].allow_clip,
        [
            ClipSide::Right,
            ClipSide::Top,
            ClipSide::Bottom,
            ClipSide::Left
        ]
    );
    assert!(matches!(
        scene.shapes[0].shape,
        Shape::Primitive {
            style: Style {
                width: Width::Thin,
                fill: Fill::Black
            },
            ..
        }
    ));
    for clip in [
        r#"["left","left"]"#,
        r#"["up"]"#,
        r#"["left","top","right","bottom","left"]"#,
    ] {
        assert!(
            load_node(&format!(
                r#"{{"type":"circle","cx":0,"cy":0,"r":1,"allow_clip":{clip}}}"#
            ))
            .is_err()
        );
    }
    for (field, value) in [("width", "huge"), ("fill", "red")] {
        assert_eq!(
            load_node(&format!(
                r#"{{"type":"circle","cx":0,"cy":0,"r":1,"{field}":"{value}"}}"#
            ))
            .unwrap_err()
            .path,
            format!("shapes[0].{field}")
        );
    }
    for container in [
        format!(r#"{{"type":"group","shapes":[{CIRCLE}]}}"#),
        format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{CIRCLE}]}}"#),
    ] {
        for field in ["width", "fill"] {
            let mut value: serde_json::Value = serde_json::from_str(&container).unwrap();
            value[field] = if field == "width" {
                "thick".into()
            } else {
                "none".into()
            };
            assert_eq!(
                load_node(&value.to_string()).unwrap_err().path,
                format!("shapes[0].{field}")
            );
        }
    }
}
#[test]
fn scene_diagnostics() {
    let error =
        load_node(r#"{"type":"group","shapes":[{"type":"circle","cx":0,"cy":0}]}"#).unwrap_err();
    assert_eq!(
        (error.kind, error.path.as_str()),
        (ErrorKind::Structure, "shapes[0].shapes[0].r")
    );
    assert!(error.detail.contains("missing"));
    let error =
        load_node(r#"{"type":"group","shapes":[{"type":"circle","cx":0,"cy":0,"r":1,"bad":0}]}"#)
            .unwrap_err();
    assert_eq!(error.path, "shapes[0].shapes[0].bad");
    let error = load_node(r#"{"type":"group","shapes":[{"r":1,"r":2}]}"#).unwrap_err();
    assert_eq!(error.path, "shapes[0].shapes[0].r");
    assert!(error.detail.contains("duplicate"));
    let error = load_node(r#"{"type":"line","from":[0,0],"angle":0,"length":-1}"#).unwrap_err();
    assert_eq!(
        error.to_string(),
        "ERROR INVALID_SHAPE path=shapes[0].length: expected a positive number"
    );
    let error = load_node(r#"{"type":"group","id":"same","shapes":[{"type":"circle","id":"same","cx":0,"cy":0,"r":1}]}"#).unwrap_err();
    assert_eq!(error.path, "shapes[0].shapes[0].id");
    assert_eq!(error.previous_path.as_deref(), Some("shapes[0].id"));
    let scene = r#"{"schema_version":1,"shapes":[{"type":"group","shapes":[{"id":"same","type":"circle","cx":0,"cy":0,"r":1}]},{"type":"group","shapes":[{"id":"same","type":"circle","cx":0,"cy":0,"r":1}]}]}"#;
    let error = parse_scene(scene.as_bytes()).unwrap_err();
    assert_eq!(error.path, "shapes[1].shapes[0].id");
    assert_eq!(
        error.previous_path.as_deref(),
        Some("shapes[0].shapes[0].id")
    );
    for (input, path) in [
        ("x", "$"),
        ("{} x", "$"),
        ("\u{feff}{}", "$"),
        (
            r#"{"schema_version":1,"shapes":[{"type":"circle",}]}"#,
            "shapes[0]",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"line","from":[0,]}]}"#,
            "shapes[0].from",
        ),
    ] {
        let error = parse_scene(input.as_bytes()).unwrap_err();
        assert_eq!((error.kind, error.path.as_str()), (ErrorKind::Syntax, path));
        assert!(error.line.is_some() && error.column.is_some());
        assert!(!error.detail.is_empty());
    }
    let input = vec![b' '; 4_194_305];
    let error = parse_scene(&input).unwrap_err();
    assert_eq!((error.kind, error.path.as_str()), (ErrorKind::Limit, "$"));
    let directory = tempfile::tempdir().unwrap();
    let sentinel = directory.path().join("out.png");
    std::fs::write(&sentinel, b"untouched").unwrap();
    assert!(load_node(CIRCLE).is_ok());
    assert!(load_node("{}").is_err());
    assert_eq!(std::fs::read(sentinel).unwrap(), b"untouched");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
