use super::*;

fn load_node(node: &str) -> Result<Scene, SceneError> {
    parse_scene(format!(r#"{{"schema_version":1,"shapes":[{node}]}}"#).as_bytes())
}
fn primitive(scene: &Scene) -> (&Style, &Primitive) {
    let Shape::Primitive { style, primitive } = &scene.shapes[0].shape else {
        panic!("expected primitive")
    };
    (style, primitive)
}
const CIRCLE: &str = r#"{"type":"circle","cx":0,"cy":0,"r":1}"#;

#[test]
fn scene_diagnostic_keys_are_unambiguous_and_single_line() {
    for key in ["x\nOK output=fake.png", "a.b", "a[0]", "quote\"", ""] {
        let quoted = serde_json::to_string(key).unwrap();
        let node = format!(r#"{{"type":"circle","cx":0,"cy":0,"r":1,{quoted}:0}}"#);
        let error = load_node(&node).unwrap_err();
        assert_eq!(error.path, format!("shapes[0][{quoted}]"));
        assert_eq!(error.to_string().lines().count(), 1);
        let input = format!(r#"{{{quoted}:1,{quoted}:2}}"#);
        let error = parse_scene(input.as_bytes()).unwrap_err();
        assert_eq!(error.path, format!("$[{quoted}]"));
        assert_eq!(error.to_string().lines().count(), 1);
    }
}

#[test]
fn scene_binary64_rounds_to_nearest() {
    let scene = load_node(r#"{"type":"circle","cx":-50.675227441983196,"cy":0,"r":1}"#).unwrap();
    let Primitive::Circle { cx, .. } = primitive(&scene).1 else {
        panic!()
    };
    assert_eq!(*cx, "-50.675227441983196".parse::<f64>().unwrap());
}

#[test]
fn scene_deep_paths_preserve_innermost_syntax_location() {
    let key = "x".repeat(20_000);
    let input = format!("{}[0,", format!("{{\"{key}\":").repeat(100));
    let error = parse_scene(input.as_bytes()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Syntax);
    assert!(error.path.ends_with("[0]") || error.path.ends_with(&key));
    assert_eq!(error.to_string().matches("line 1 column").count(), 1);
}

#[test]
fn scene_root_contract() {
    let scene = load_node(CIRCLE).unwrap();
    assert_eq!(scene.schema_version, 1);
    assert_eq!(scene.canvas, Canvas::Landscape);
    for (name, canvas) in [("portrait", Canvas::Portrait), ("square", Canvas::Square)] {
        let json = format!(r#"{{"schema_version":1,"canvas":"{name}","shapes":[{CIRCLE}]}}"#);
        assert_eq!(parse_scene(json.as_bytes()).unwrap().canvas, canvas);
    }
    for (input, path) in [
        (r#"{"schema_version":1,"shapes":[]}"#.to_owned(), "shapes"),
        (r#"{"schema_version":1}"#.to_owned(), "shapes"),
        (format!(r#"{{"shapes":[{CIRCLE}]}}"#), "schema_version"),
        (
            format!(r#"{{"schema_version":2,"shapes":[{CIRCLE}]}}"#),
            "schema_version",
        ),
        (
            format!(r#"{{"schema_version":1,"canvas":"wide","shapes":[{CIRCLE}]}}"#),
            "canvas",
        ),
    ] {
        let error = parse_scene(input.as_bytes()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Structure, "{input}: {error}");
        assert_eq!(error.path, path, "{input}: {error}");
    }
}
#[test]
fn scene_strict_json() {
    for node in [CIRCLE, r#"{"r":1,"cy":0,"cx":0,"type":"circle"}"#] {
        assert!(load_node(node).is_ok());
    }
    let invalid = [
        (
            r#"{"schema_version":1,"canvas":"square","canvas":"square","shapes":[]}"#,
            "canvas",
        ),
        (
            r#"{"schema_version":1,"extra":0,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}"#,
            "extra",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"circle","type":"circle"}]}"#,
            "shapes[0].type",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1,"r":1}]}"#,
            "shapes[0].r",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"id":"a","\u0069d":"b"}]}"#,
            "shapes[0].id",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"group","shapes":[{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{"r":1,"r":1}]}]}]}"#,
            "shapes[0].shapes[0].shapes[0].r",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1,"extra":true}]}"#,
            "shapes[0].extra",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"wat"}]}"#,
            "shapes[0].type",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"circle","cx":0,"cy":0}]}"#,
            "shapes[0].r",
        ),
        (
            r#"{"schema_version":1,"canvas":null,"shapes":[]}"#,
            "canvas",
        ),
        (r#"{"schema_version":1,"shapes":null}"#, "shapes"),
    ];
    for (input, path) in invalid {
        let error = parse_scene(input.as_bytes()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Structure, "{input}: {error}");
        assert_eq!(error.path, path);
    }
    for field in ["id", "width", "fill", "allow_clip", "r", "cx", "cy"] {
        let mut value: serde_json::Value = serde_json::from_str(CIRCLE).unwrap();
        value[field] = serde_json::Value::Null;
        assert!(load_node(&value.to_string()).is_err(), "{field}");
    }
    for coordinates in ["[]", "[0]", "[0,1,2]", "[0,null]", "[0,\"x\"]", "{}"] {
        let error = load_node(&format!(
            r#"{{"type":"line","from":{coordinates},"to":[1,1]}}"#
        ))
        .unwrap_err();
        assert!(error.path.starts_with("shapes[0].from"));
    }
    let syntax_cases = [
        b"\xef\xbb\xbf{}".as_slice(),
        b"\xff",
        b"/*x*/{}",
        b"{} {}",
        b"{\"schema_version\":1,}",
        b"{\"bad\":\"\xff\"}",
    ];
    for input in syntax_cases {
        let error = parse_scene(input).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Syntax);
        assert!(error.line.is_some() && error.column.is_some());
    }
    let deep = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    let error = parse_scene(deep.as_bytes()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Syntax);
    assert!(error.detail.contains("recursion"));
}
#[test]
fn scene_structural_defaults() {
    let scene = load_node(CIRCLE).unwrap();
    let (style, _) = primitive(&scene);
    assert_eq!((style.width, style.fill), (Width::Thick, Fill::None));
    assert_eq!(scene.shapes[0].id, None);
    assert!(scene.shapes[0].allow_clip.is_empty());
    let ellipse = load_node(r#"{"type":"ellipse","cx":0,"cy":0,"rx":1,"ry":2}"#).unwrap();
    assert!(matches!(
        primitive(&ellipse).1,
        Primitive::Ellipse { angle: 0.0, .. }
    ));
    let arc =
        load_node(r#"{"type":"arc","cx":0,"cy":0,"rx":1,"ry":2,"start":0,"sweep":90}"#).unwrap();
    assert!(matches!(
        primitive(&arc).1,
        Primitive::Arc {
            angle: 0.0,
            chord: false,
            ..
        }
    ));
}
#[test]
fn scene_container_rules() {
    for count in ["0", "257", "6.5"] {
        let node = format!(
            r#"{{"type":"repeat_rotate","center":[0,0],"count":{count},"shapes":[{CIRCLE}]}}"#
        );
        assert_eq!(load_node(&node).unwrap_err().path, "shapes[0].count");
    }
    assert!(load_node(r#"{"type":"repeat_rotate","center":[0,0],"count":2,"shapes":[{"type":"circle","id":"child","cx":0,"cy":0,"r":1}]}"#).is_ok());
    let group = load_node(&format!(r#"{{"type":"group","shapes":[{{"type":"repeat_rotate","center":[-1,2],"count":6,"shapes":[{CIRCLE}]}}]}}"#)).unwrap();
    let Shape::Group {
        at,
        rotate,
        scale,
        flip_x,
        shapes,
    } = &group.shapes[0].shape
    else {
        panic!()
    };
    assert_eq!(
        (*at, *rotate, *scale, *flip_x),
        ([0.0, 0.0], 0.0, 1.0, false)
    );
    let Shape::RepeatRotate {
        center,
        count,
        start,
        step,
        ..
    } = &shapes[0].shape
    else {
        panic!()
    };
    assert_eq!(
        (*center, *count, *start, *step),
        ([-1.0, 2.0], 6, 0.0, None)
    );
    for step in ["0", "-30"] {
        let scene = load_node(&format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"step":{step},"shapes":[{CIRCLE}]}}"#)).unwrap();
        let Shape::RepeatRotate { step: stored, .. } = scene.shapes[0].shape else {
            panic!()
        };
        assert_eq!(stored, Some(step.parse().unwrap()));
    }
    let group = format!(r#"{{"type":"group","shapes":[{CIRCLE}]}}"#);
    let repeat =
        format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{CIRCLE}]}}"#);
    for node in [
        format!(r#"{{"type":"group","shapes":[{group}]}}"#),
        format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{group}]}}"#),
        format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{repeat}]}}"#),
        r#"{"type":"group","shapes":[]}"#.into(),
        r#"{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[]}"#.into(),
    ] {
        assert!(load_node(&node).is_err(), "{node}");
    }
    for field in ["center", "count"] {
        let mut value: serde_json::Value = serde_json::from_str(&repeat).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert_eq!(
            load_node(&value.to_string()).unwrap_err().path,
            format!("shapes[0].{field}")
        );
    }
    for field in ["step", "start", "center", "count"] {
        let mut value: serde_json::Value = serde_json::from_str(&repeat).unwrap();
        value[field] = serde_json::Value::Null;
        assert!(load_node(&value.to_string()).is_err());
    }
    for (field, value) in [
        ("at", "null"),
        ("rotate", "null"),
        ("scale", "null"),
        ("flip_x", "null"),
        ("scale", "0"),
        ("scale", "-1"),
    ] {
        assert!(
            load_node(&format!(
                r#"{{"type":"group","{field}":{value},"shapes":[{CIRCLE}]}}"#
            ))
            .is_err()
        );
    }
}
#[test]
fn scene_numeric_policy() {
    for count in [
        "6",
        "6.0",
        "6e0",
        "6.00000000000000000001",
        "256.00000000000000001",
        "256",
    ] {
        let scene = load_node(&format!(
            r#"{{"type":"repeat_rotate","center":[0,0],"count":{count},"shapes":[{CIRCLE}]}}"#
        ))
        .unwrap();
        let Shape::RepeatRotate { count: stored, .. } = scene.shapes[0].shape else {
            panic!()
        };
        assert_eq!(stored, if count.starts_with("256") { 256 } else { 6 });
    }
    for version in ["1", "1.0", "1e0", "1.00000000000000000001"] {
        assert!(
            parse_scene(
                format!(r#"{{"schema_version":{version},"shapes":[{CIRCLE}]}}"#).as_bytes()
            )
            .is_ok()
        );
    }
    for version in ["2", "1.5", "\"1\""] {
        let input = format!(r#"{{"schema_version":{version},"shapes":[{CIRCLE}]}}"#);
        let error = parse_scene(input.as_bytes()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Structure, "{input}: {error}");
        assert_eq!(error.path, "schema_version", "{input}: {error}");
    }
    for count in ["0", "257", "6.5", "\"6\"", "true", "{}", "1e400"] {
        assert!(
            load_node(&format!(
                r#"{{"type":"repeat_rotate","center":[0,0],"count":{count},"shapes":[{CIRCLE}]}}"#
            ))
            .is_err()
        );
    }
    for number in ["\"1\"", "true", "{}", "1e400", "null"] {
        assert!(
            load_node(&format!(
                r#"{{"type":"circle","cx":{number},"cy":0,"r":1}}"#
            ))
            .is_err()
        );
    }
    assert!(load_node(r#"{"type":"circle","cx":0,"cy":0,"r":1e-400}"#).is_err());
    assert!(
        load_node(r#"{"type":"circle","cx":18446744073709551615,"cy":-9223372036854775808,"r":1}"#)
            .is_ok()
    );
}
#[test]
fn scene_limits() {
    let base = format!(r#"{{"schema_version":1,"shapes":[{CIRCLE}]}}"#);
    let mut input = base.into_bytes();
    input.resize(MAX_INPUT_BYTES, b' ');
    assert!(parse_scene(&input).is_ok());
    input.push(b' ');
    let error = parse_scene(&input).unwrap_err();
    assert_eq!((error.kind, error.path.as_str()), (ErrorKind::Limit, "$"));
    let valid_path = format!("M0 0L1 1{}", " ".repeat(65528));
    assert_eq!(valid_path.len(), 65536);
    for (body, is_valid) in [
        (valid_path, true),
        ("x".repeat(65537), false),
        (format!("{}x", "é".repeat(32768)), false),
        (format!("\\u004d0 0L1 1{}", " ".repeat(65528)), true),
        ("\\u0061".repeat(65537), false),
        (format!("{}x", "\\u00e9".repeat(32768)), false),
    ] {
        let result = load_node(&format!(r#"{{"type":"path","d":"{body}"}}"#));
        assert_eq!(result.is_ok(), is_valid);
        if let Err(error) = result {
            assert_eq!(
                (error.kind, error.path.as_str()),
                (ErrorKind::Limit, "shapes[0].d")
            );
        }
    }
    // 1 direct + (99 direct + repeat(100 * 99)) in a group = 10000.
    let children = vec![CIRCLE; 99].join(",");
    let repeat =
        format!(r#"{{"type":"repeat_rotate","center":[0,0],"count":100,"shapes":[{children}]}}"#);
    let group = format!(r#"{{"type":"group","shapes":[{children},{repeat}]}}"#);
    let exact = format!(r#"{{"schema_version":1,"shapes":[{CIRCLE},{group}]}}"#);
    assert!(parse_scene(exact.as_bytes()).is_ok());
    let over = format!(r#"{{"schema_version":1,"shapes":[{CIRCLE},{CIRCLE},{group}]}}"#);
    assert_eq!(
        parse_scene(over.as_bytes()).unwrap_err().kind,
        ErrorKind::Limit
    );
    let direct = vec![CIRCLE; 10000].join(",");
    assert!(
        parse_scene(format!(r#"{{"schema_version":1,"shapes":[{direct}]}}"#).as_bytes()).is_ok()
    );
    let over = format!("{direct},{CIRCLE}");
    assert_eq!(
        parse_scene(format!(r#"{{"schema_version":1,"shapes":[{over}]}}"#).as_bytes())
            .unwrap_err()
            .kind,
        ErrorKind::Limit
    );
    assert!(load_node(&format!(r#"{{"type":"group","shapes":[{direct}]}}"#)).is_ok());
    assert_eq!(
        load_node(&format!(r#"{{"type":"group","shapes":[{over}]}}"#))
            .unwrap_err()
            .kind,
        ErrorKind::Limit
    );
    assert!(
        load_node(&format!(
            r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{direct}]}}"#
        ))
        .is_ok()
    );
    assert_eq!(
        load_node(&format!(
            r#"{{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{over}]}}"#
        ))
        .unwrap_err()
        .kind,
        ErrorKind::Limit
    );
}
#[test]
fn scene_examples() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/layout-sketch/assets/examples/"
    );
    let cases = [
        ("01-basic.json", Canvas::Landscape),
        ("02-pulley.json", Canvas::Landscape),
        ("03-curves.json", Canvas::Landscape),
        ("04-group-repeat.json", Canvas::Landscape),
        ("05-portrait.json", Canvas::Portrait),
        ("06-square.json", Canvas::Square),
    ];
    let mut seen = std::collections::HashSet::new();
    for (file, canvas) in cases {
        let input = std::fs::read(format!("{directory}{file}")).unwrap();
        let scene = parse_scene(&input).unwrap();
        assert_eq!(scene.canvas, canvas);
        collect_variants(&scene.shapes, &mut seen);
    }
    assert_eq!(seen.len(), 11);
    let scene =
        parse_scene(&std::fs::read(format!("{directory}04-group-repeat.json")).unwrap()).unwrap();
    assert_eq!(
        scene
            .shapes
            .iter()
            .map(|node| node.id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["wheel-assembly", "radial-rays", "sun-center"]
    );
    let Shape::Group {
        at,
        rotate,
        scale,
        flip_x,
        shapes,
    } = &scene.shapes[0].shape
    else {
        panic!()
    };
    assert_eq!(
        (*at, *rotate, *scale, *flip_x),
        ([45.0, 50.0], 30.0, 1.4, true)
    );
    assert_eq!(shapes[0].id.as_deref(), Some("rim"));
    let Shape::RepeatRotate {
        center,
        count,
        shapes,
        ..
    } = &shapes[1].shape
    else {
        panic!()
    };
    assert_eq!((*center, *count), ([0.0, 0.0], 6));
    assert_eq!(shapes[0].id.as_deref(), Some("spoke-segment"));
    let Shape::Primitive {
        primitive: Primitive::Line(Line::Endpoints { from, to }),
        style,
    } = &shapes[0].shape
    else {
        panic!()
    };
    assert_eq!(
        (*from, *to, style.width),
        ([0.0, 0.0], [18.0, 0.0], Width::Thin)
    );
}
fn collect_variants(nodes: &[Node], seen: &mut std::collections::HashSet<&'static str>) {
    for node in nodes {
        let name = match &node.shape {
            Shape::Group { shapes, .. } => {
                collect_variants(shapes, seen);
                "group"
            }
            Shape::RepeatRotate { shapes, .. } => {
                collect_variants(shapes, seen);
                "repeat_rotate"
            }
            Shape::Primitive { primitive, .. } => match primitive {
                Primitive::Line(_) => "line",
                Primitive::Polyline { .. } => "polyline",
                Primitive::Polygon { .. } => "polygon",
                Primitive::Rect { .. } => "rect",
                Primitive::Circle { .. } => "circle",
                Primitive::Ellipse { .. } => "ellipse",
                Primitive::Arc { .. } => "arc",
                Primitive::Curve { .. } => "curve",
                Primitive::Path { .. } => "path",
            },
        };
        seen.insert(name);
    }
}
