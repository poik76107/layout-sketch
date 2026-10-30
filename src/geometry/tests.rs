use super::*;
use crate::scene::{Shape, parse_scene};
fn scene(node: serde_json::Value) -> crate::scene::Scene {
    let json = serde_json::json!({"schema_version":1,"shapes":[node]});
    parse_scene(json.to_string().as_bytes()).unwrap()
}
fn build(node: serde_json::Value, scale: f64) -> Geometry {
    let scene = scene(node);
    let node = &scene.shapes[0];
    let Shape::Primitive { primitive, style } = &node.shape else {
        panic!()
    };
    build_primitive(primitive, style, "shapes[0]", node.id.as_deref(), scale).unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn canvas_circle() {
    let mut expected = None;
    for canvas in ["landscape", "portrait", "square"] {
        let input = serde_json::json!({"schema_version":1,"canvas":canvas,"shapes":[{"type":"circle","cx":0,"cy":0,"r":10}]});
        let scene = parse_scene(input.to_string().as_bytes()).unwrap();
        let Shape::Primitive { primitive, style } = &scene.shapes[0].shape else {
            panic!()
        };
        let geometry = build_primitive(primitive, style, "shapes[0]", None, 10.24).unwrap();
        if let Some(previous) = &expected {
            assert_eq!(previous, &geometry.segments);
        }
        expected = Some(geometry.segments);
    }
}
#[test]
fn geometry_primitives() {
    canvas_circle();
    let line = build(
        serde_json::json!({"id":"a","type":"line","from":[10,20],"angle":90,"length":5,"width":"thin"}),
        10.24,
    );
    assert_eq!(line.id.as_deref(), Some("a"));
    assert_eq!(line.source_path, "shapes[0]");
    assert_eq!(line.width, crate::scene::Width::Thin);
    let Segment::Line(end) = line.segments[1] else {
        panic!()
    };
    near(end[0], 10.0);
    near(end[1], 25.0);
    let cases = [
        (
            serde_json::json!({"type":"line","from":[0,0],"to":[1,1]}),
            vec![Segment::Move([0.0, 0.0]), Segment::Line([1.0, 1.0])],
        ),
        (
            serde_json::json!({"type":"polyline","pts":[[0,0],[1,1],[2,0]]}),
            vec![
                Segment::Move([0.0, 0.0]),
                Segment::Line([1.0, 1.0]),
                Segment::Line([2.0, 0.0]),
            ],
        ),
        (
            serde_json::json!({"type":"curve","from":[0,0],"ctrl1":[0,1],"ctrl2":[1,1],"to":[1,0]}),
            vec![
                Segment::Move([0.0, 0.0]),
                Segment::Cubic([0.0, 1.0], [1.0, 1.0], [1.0, 0.0]),
            ],
        ),
        (
            serde_json::json!({"type":"path","d":"M0 0L1 1Q1 2 2 0C2 1 3 1 3 0Z"}),
            vec![
                Segment::Move([0.0, 0.0]),
                Segment::Line([1.0, 1.0]),
                Segment::Quadratic([1.0, 2.0], [2.0, 0.0]),
                Segment::Cubic([2.0, 1.0], [3.0, 1.0], [3.0, 0.0]),
                Segment::Close,
            ],
        ),
    ];
    for (item, expected) in cases {
        assert_eq!(build(item, 10.24).segments, expected);
    }
    let rect = build(
        serde_json::json!({"type":"rect","x":0,"y":0,"w":2,"h":1,"fill":"white","width":"thin"}),
        10.24,
    );
    assert_eq!(
        rect.segments,
        [
            Segment::Move([0.0, 0.0]),
            Segment::Line([2.0, 0.0]),
            Segment::Line([2.0, 1.0]),
            Segment::Line([0.0, 1.0]),
            Segment::Close
        ]
    );
    assert_eq!(
        (rect.fill, rect.width, rect.fill_rule),
        (
            Fill::White,
            crate::scene::Width::Thin,
            tiny_skia::FillRule::EvenOdd
        )
    );
    for node in [
        serde_json::json!({"type":"circle","cx":0,"cy":0,"r":1}),
        serde_json::json!({"type":"ellipse","cx":0,"cy":0,"rx":2,"ry":1}),
    ] {
        let geometry = build(node, 10.24);
        assert_eq!(geometry.segments.last(), Some(&Segment::Close));
        let (Segment::Move(start), Segment::Line(end)) = (
            &geometry.segments[0],
            &geometry.segments[geometry.segments.len() - 2],
        ) else {
            panic!()
        };
        near(start[0], end[0]);
        near(start[1], end[1]);
    }
    let open_arc = build(
        serde_json::json!({"type":"arc","cx":0,"cy":0,"rx":2,"ry":1,"start":0,"sweep":90}),
        10.24,
    );
    assert!(matches!(open_arc.segments.last(), Some(Segment::Line(_))));
    let a = build(
        serde_json::json!({"type":"polygon","pts":[[0,0],[1,0],[0,1]]}),
        10.24,
    );
    let b = build(
        serde_json::json!({"type":"polygon","pts":[[0,0],[1,0],[0,1],[0,0]]}),
        10.24,
    );
    assert_eq!(a.segments, b.segments);
    assert_eq!(a.segments.last(), Some(&Segment::Close));
    let original = scene(serde_json::json!({"type":"polygon","pts":[[0,0],[1,0],[0,1],[0,0]]}));
    let Shape::Primitive {
        primitive: crate::scene::Primitive::Polygon { pts },
        style,
    } = &original.shapes[0].shape
    else {
        panic!()
    };
    let source = pts.clone();
    let Shape::Primitive { primitive, .. } = &original.shapes[0].shape else {
        panic!()
    };
    build_primitive(primitive, style, "shapes[0]", None, 10.24).unwrap();
    assert_eq!(*pts, source);
    assert_eq!(pts.len(), 4);
    let bow = build(
        serde_json::json!({"type":"polygon","pts":[[0,0],[1,1],[0,1],[1,0]],"fill":"black"}),
        10.24,
    );
    assert_eq!(bow.fill, Fill::Black);
    assert_eq!(bow.fill_rule, tiny_skia::FillRule::EvenOdd);
    assert_eq!(
        bow.segments,
        [
            Segment::Move([0.0, 0.0]),
            Segment::Line([1.0, 1.0]),
            Segment::Line([0.0, 1.0]),
            Segment::Line([1.0, 0.0]),
            Segment::Close
        ]
    );
    let invalid =
        serde_json::json!({"schema_version":1,"shapes":[{"type":"rect","x":0,"y":0,"w":0,"h":1}]});
    let invalid_error = parse_scene(invalid.to_string().as_bytes()).unwrap_err();
    assert_eq!(
        (invalid_error.kind, invalid_error.path.as_str()),
        (ErrorKind::Shape, "shapes[0].w")
    );
    let scale_error = build_primitive(primitive, style, "shapes[0]", None, 0.0).unwrap_err();
    assert_eq!(
        (scale_error.kind, scale_error.path.as_str()),
        (ErrorKind::Shape, "shapes[0]")
    );
}
#[test]
fn geometry_bounds() {
    let line = build(
        serde_json::json!({"type":"line","from":[-2,3],"to":[4,-5]}),
        10.24,
    );
    assert_eq!(line.bounds.min, [-2.0, -5.0]);
    assert_eq!(line.bounds.max, [4.0, 3.0]);
    let rect = build(
        serde_json::json!({"type":"rect","x":-2,"y":3,"w":4,"h":5}),
        10.24,
    );
    assert_eq!(rect.bounds.min, [-2.0, 3.0]);
    assert_eq!(rect.bounds.max, [2.0, 8.0]);
    let q = build(serde_json::json!({"type":"path","d":"M0 0Q1 2 2 0"}), 10.24);
    near(q.bounds.max[1], 1.0);
    let c = build(
        serde_json::json!({"type":"curve","from":[0,0],"ctrl1":[0,3],"ctrl2":[3,3],"to":[3,0]}),
        10.24,
    );
    near(c.bounds.max[1], 2.25);
    let elevated = build(
        serde_json::json!({"type":"path","d":"M0 0 C0.6666666666666666 1.3333333333333333 1.3333333333333333 1.3333333333333333 2 0"}),
        10.24,
    );
    near(elevated.bounds.max[1], 1.0);
    for (start, control, end) in [
        (
            [7.195882415616332, -2.113973232873265],
            [-0.388242519010165, -6.22100228344509],
            [-6.490756419338675, 1.8017933438838423],
        ),
        ([-9.1, 2.3], [8.7, -9.6], [4.2, 5.9]),
        ([3.4, -8.1], [-7.2, 9.3], [-4.8, -2.7]),
        ([1.1, 6.2], [9.9, -7.8], [-8.3, 3.6]),
    ] {
        let control1 = [
            start[0] + 2.0 / 3.0 * (control[0] - start[0]),
            start[1] + 2.0 / 3.0 * (control[1] - start[1]),
        ];
        let control2 = [
            end[0] + 2.0 / 3.0 * (control[0] - end[0]),
            end[1] + 2.0 / 3.0 * (control[1] - end[1]),
        ];
        let curve = build(
            serde_json::json!({"type":"curve","from":start,"ctrl1":control1,"ctrl2":control2,"to":end}),
            10.24,
        );
        for index in 0..=20_000 {
            let t = index as f64 / 20_000.0;
            for axis in 0..2 {
                let value = (1.0 - t).powi(3) * start[axis]
                    + 3.0 * (1.0 - t).powi(2) * t * control1[axis]
                    + 3.0 * (1.0 - t) * t * t * control2[axis]
                    + t.powi(3) * end[axis];
                assert!(
                    value >= curve.bounds.min[axis] - 1e-9
                        && value <= curve.bounds.max[axis] + 1e-9,
                    "sample {value}, bounds {:?}",
                    curve.bounds
                );
            }
        }
    }
    let m = build(
        serde_json::json!({"type":"path","d":"M0 0L1 1 M-2 -3L3 2Z"}),
        10.24,
    );
    assert_eq!(m.bounds.min, [-2.0, -3.0]);
    assert_eq!(m.bounds.max, [3.0, 2.0]);
}
#[test]
fn geometry_arcs() {
    let chord = build(
        serde_json::json!({"type":"arc","cx":20,"cy":20,"rx":10,"ry":10,"start":0,"sweep":90,"chord":true}),
        10.24,
    );
    assert_eq!(chord.segments.last(), Some(&Segment::Close));
    let Segment::Move(start) = chord.segments[0] else {
        panic!()
    };
    let Segment::Line(end) = chord.segments[chord.segments.len() - 2] else {
        panic!()
    };
    near(start[0], 30.0);
    near(start[1], 20.0);
    near(end[0], 20.0);
    near(end[1], 30.0);
    assert!(
        !chord
            .segments
            .iter()
            .any(|segment| matches!(segment, Segment::Line(point) if *point == [20.0,20.0]))
    );
    let loaded =
        scene(serde_json::json!({"type":"arc","cx":0,"cy":0,"rx":2,"ry":1,"start":0,"sweep":90}));
    let Shape::Primitive { primitive, style } = &loaded.shapes[0].shape else {
        panic!()
    };
    assert!(build_primitive(primitive, style, "shapes[0]", None, f64::INFINITY).is_err());
    assert!(build_primitive(primitive, style, "shapes[0]", None, f64::MAX).is_err());
    for (sweep, expected) in [(90, 30.0), (-90, 10.0)] {
        let arc = build(
            serde_json::json!({"type":"arc","cx":20,"cy":20,"rx":10,"ry":10,"start":0,"sweep":sweep}),
            10.24,
        );
        let Segment::Move(start) = arc.segments[0] else {
            panic!()
        };
        near(start[0], 30.0);
        near(start[1], 20.0);
        let Segment::Line(end) = arc.segments.last().unwrap() else {
            panic!()
        };
        near(end[0], 20.0);
        near(end[1], expected);
    }
    for sweep in [360, -360] {
        for chord in [true, false] {
            let arc = build(
                serde_json::json!({"type":"arc","cx":20,"cy":20,"rx":20,"ry":10,"angle":90,"start":0,"sweep":sweep,"chord":chord}),
                10.24,
            );
            assert_eq!(matches!(arc.segments.last(), Some(Segment::Close)), chord);
            let Segment::Move(start) = arc.segments[0] else {
                panic!()
            };
            near(start[0], 20.0);
            near(start[1], 40.0);
            assert!(
                !arc.segments
                    .iter()
                    .any(|s| matches!(s, Segment::Line(p) if *p == [20.0, 20.0]))
            );
        }
    }
}
#[test]
fn geometry_arc_accuracy() {
    canvas_circle();
    for scale in [10.24, 1024.0, 10240.0] {
        for (kind, rx, ry, angle, start, sweep) in [
            ("circle", 10.0_f64, 10.0, 0.0_f64, 0.0, 360.0),
            ("ellipse", 20.0, 10.0, 45.0, 0.0, 360.0),
            ("arc", 20.0, 10.0, 45.0, 20.0, 180.0),
            ("arc", 20.0, 10.0, 45.0, 20.0, -180.0),
        ] {
            let node = match kind {
                "circle" => serde_json::json!({"type":kind,"cx":0,"cy":0,"r":rx}),
                "ellipse" => {
                    serde_json::json!({"type":kind,"cx":0,"cy":0,"rx":rx,"ry":ry,"angle":angle})
                }
                _ => {
                    serde_json::json!({"type":kind,"cx":0,"cy":0,"rx":rx,"ry":ry,"angle":angle,"start":start,"sweep":sweep})
                }
            };
            let geometry = build(node, scale);
            let points: Vec<Point> = geometry
                .segments
                .iter()
                .filter_map(|s| match s {
                    Segment::Move(p) | Segment::Line(p) => Some(*p),
                    _ => None,
                })
                .collect();
            for index in 0..=4096 {
                let f = index as f64 / 4096.0;
                let parameter = (start + sweep * f).to_radians();
                let (sin, cos) = parameter.sin_cos();
                let (rotation_sin, rotation_cos) = angle.to_radians().sin_cos();
                let point = [
                    rx * cos * rotation_cos - ry * sin * rotation_sin,
                    rx * cos * rotation_sin + ry * sin * rotation_cos,
                ];
                let segment = (f * (points.len() - 1) as f64)
                    .floor()
                    .min((points.len() - 2) as f64) as usize;
                let t = f * (points.len() - 1) as f64 - segment as f64;
                let interp = [
                    points[segment][0] * (1.0 - t) + points[segment + 1][0] * t,
                    points[segment][1] * (1.0 - t) + points[segment + 1][1] * t,
                ];
                assert!((point[0] - interp[0]).hypot(point[1] - interp[1]) * scale <= 0.2500001);
            }
        }
    }
    let node = serde_json::json!({"type":"circle","cx":0,"cy":0,"r":10});
    assert!(build(node.clone(), 10240.0).segments.len() > build(node, 10.24).segments.len());
    let scene = scene(serde_json::json!({"type":"circle","cx":0,"cy":0,"r":10}));
    let Shape::Primitive { primitive, style } = &scene.shapes[0].shape else {
        panic!()
    };
    for scale in [0.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(build_primitive(primitive, style, "shapes[0]", None, scale).is_err());
    }
}
#[test]
fn geometry_finite() {
    for node in [
        serde_json::json!({"type":"line","from":[1e307,0],"angle":0,"length":1.7e308}),
        serde_json::json!({"type":"rect","x":1e307,"y":0,"w":1.7e308,"h":1}),
        serde_json::json!({"type":"circle","cx":1e307,"cy":0,"r":1.7e308}),
        serde_json::json!({"type":"arc","cx":1e307,"cy":0,"rx":1.7e308,"ry":1,"start":0,"sweep":90}),
        serde_json::json!({"type":"line","from":[1e308,0],"to":[0,0]}),
    ] {
        let json = serde_json::json!({"schema_version":1,"shapes":[node]});
        let error = parse_scene(json.to_string().as_bytes()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Shape);
        assert!(error.path.starts_with("shapes[0]"));
    }
    let valid =
        scene(serde_json::json!({"type":"line","from":[-1.5,-2.0],"angle":1e300,"length":2}));
    let Shape::Primitive { primitive, style } = &valid.shapes[0].shape else {
        panic!()
    };
    for scale in [0.0, f64::NAN, f64::INFINITY] {
        assert!(build_primitive(primitive, style, "shapes[0]", None, scale).is_err());
    }
    let nested = serde_json::json!({"schema_version":1,"shapes":[{"type":"group","shapes":[{"type":"line","from":[1e307,0],"angle":0,"length":1.7e308}]}]});
    let error = parse_scene(nested.to_string().as_bytes()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Shape);
    assert_eq!(error.path, "shapes[0].shapes[0].length");
}

#[test]
fn geometry_local_contract() {
    for scale in [10.24, 20.48] {
        let line = build(
            serde_json::json!({"id":"pin","type":"line","from":[-1,2],"to":[3,4],"width":"thin"}),
            scale,
        );
        assert_eq!(
            line.segments,
            [Segment::Move([-1.0, 2.0]), Segment::Line([3.0, 4.0])]
        );
        assert_eq!(
            line.bounds,
            Bounds {
                min: [-1.0, 2.0],
                max: [3.0, 4.0]
            }
        );
        assert_eq!(line.id.as_deref(), Some("pin"));
        assert_eq!(line.source_path, "shapes[0]");
        let path = build(
            serde_json::json!({"type":"path","d":"M0 0Q1 2 2 0C2 0 3 0 4 0"}),
            scale,
        );
        assert_eq!(
            path.segments,
            [
                Segment::Move([0.0, 0.0]),
                Segment::Quadratic([1.0, 2.0], [2.0, 0.0]),
                Segment::Cubic([2.0, 0.0], [3.0, 0.0], [4.0, 0.0])
            ]
        );
        assert_eq!(
            path.bounds,
            Bounds {
                min: [0.0, 0.0],
                max: [4.0, 1.0]
            }
        );
    }
}

fn expand(node: serde_json::Value) -> Vec<ExpandedPrimitive> {
    expand_scene(&scene(node)).unwrap()
}
fn assert_point(actual: Point, expected: Point) {
    near(actual[0], expected[0]);
    near(actual[1], expected[1]);
}
fn endpoints(item: &ExpandedPrimitive, from: Point, to: Point) {
    let [Segment::Move(a), Segment::Line(b)] = item.geometry.segments.as_slice() else {
        panic!("expected line")
    };
    assert_point(*a, from);
    assert_point(*b, to);
}
#[test]
fn geometry_group_transform() {
    check_group_cases();
    let items = expand(
        serde_json::json!({"type":"group","flip_x":true,"scale":2,"rotate":90,"at":[10,10],"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}),
    );
    endpoints(&items[0], [102.4, 102.4], [102.4, 61.44]);
    let original = expand(serde_json::json!({"type":"line","from":[-1,0.5],"to":[2,3]}));
    let grouped = expand(
        serde_json::json!({"type":"group","shapes":[{"type":"line","from":[-1,0.5],"to":[2,3]}]}),
    );
    assert_eq!(original[0].geometry.segments, grouped[0].geometry.segments);
}
#[test]
fn geometry_repeat_rotate() {
    check_repeat_cases();
    let items = expand(
        serde_json::json!({"type":"repeat_rotate","center":[10,10],"count":4,"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}),
    );
    for (item, end) in items.iter().zip([
        [122.88, 102.4],
        [102.4, 122.88],
        [81.92, 102.4],
        [102.4, 81.92],
    ]) {
        endpoints(item, [102.4, 102.4], end);
    }
    assert_eq!(items.len(), 4);
    let six = expand(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":6,"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}),
    );
    assert_eq!(six.len(), 6);
    for (index, item) in six.iter().enumerate() {
        let angle = index as f64 * std::f64::consts::TAU / 6.0;
        endpoints(item, [0.0, 0.0], [20.48 * angle.cos(), 20.48 * angle.sin()]);
    }
}
#[test]
fn geometry_expansion_order() {
    check_order_cases();
    let items = expand(
        serde_json::json!({"type":"group","flip_x":true,"scale":2,"rotate":90,"at":[10,10],"shapes":[{"type":"repeat_rotate","center":[3,4],"count":2,"step":90,"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}]}),
    );
    endpoints(&items[0], [20.48, 40.96], [20.48, 0.0]);
    endpoints(&items[1], [20.48, 40.96], [-20.48, 40.96]);
    assert_eq!(items[1].repeat_index, Some(1));
}
#[test]
fn geometry_expanded_paths() {
    check_path_cases();
    let items = expand(
        serde_json::json!({"type":"group","rotate":90,"shapes":[{"type":"path","d":"M0 0Q1 2 2 0"}]}),
    );
    let bounds = &items[0].geometry.bounds;
    assert_point(bounds.min, [-10.24, 0.0]);
    assert_point(bounds.max, [0.0, 20.48]);
    let Segment::Quadratic(control, _) = items[0].geometry.segments[1] else {
        panic!()
    };
    assert_point(control, [-20.48, 10.24]);
}
#[test]
fn geometry_expanded_styles() {
    check_style_cases();
    let items = expand(
        serde_json::json!({"type":"group","scale":2,"shapes":[{"type":"rect","x":0,"y":0,"w":1,"h":1,"width":"thin","fill":"white"}]}),
    );
    assert_eq!(items[0].geometry.width, crate::scene::Width::Thin);
    assert_eq!(items[0].geometry.fill, Fill::White);
    assert_point(items[0].geometry.bounds.max, [20.48, 20.48]);
}
#[test]
fn geometry_expanded_accuracy() {
    check_repeat_phase_accuracy();
    check_accuracy_cases();
    let items = expand(
        serde_json::json!({"type":"group","scale":1000,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}),
    );
    assert!(items[0].geometry.segments.len() > 100);
    for sample in 0..=4096 {
        let position = sample as f64 / 4096.0;
        let lines = items[0].geometry.segments.len() - 2;
        let segment = ((position * lines as f64) as usize).min(lines - 1);
        let fraction = position * lines as f64 - segment as f64;
        let point = |index| match items[0].geometry.segments[index] {
            Segment::Move(p) | Segment::Line(p) => p,
            _ => panic!("expected ellipse vertex"),
        };
        let a = point(segment);
        let b = point(segment + 1);
        let angle = position * std::f64::consts::TAU;
        let expected = [10240.0 * angle.cos(), 10240.0 * angle.sin()];
        let actual = [
            a[0] * (1.0 - fraction) + b[0] * fraction,
            a[1] * (1.0 - fraction) + b[1] * fraction,
        ];
        assert!((actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 0.2500001);
    }

    let source = scene(
        serde_json::json!({"type":"group","at":[1e16,0],"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}),
    );
    assert_eq!(expand_scene(&source).unwrap_err().kind, ErrorKind::Shape);
}
#[test]
fn geometry_expansion_limits() {
    check_limit_cases();
    let child = serde_json::json!({"type":"line","from":[0,0],"to":[1,0]});
    let input = serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":100,"shapes":vec![child;100]});
    assert_eq!(expand(input).len(), 10000);
}
#[test]
fn geometry_expansion_finite() {
    check_finite_cases();
    let source = scene(
        serde_json::json!({"type":"group","scale":1e308,"shapes":[{"type":"line","from":[0,0],"to":[1,0]}]}),
    );
    let failure = expand_scene(&source).unwrap_err();
    assert_eq!(failure.kind, ErrorKind::Shape);
    assert_eq!(failure.path, "shapes[0].scale");
    assert_eq!(failure.detail, "final scale is not finite");
}
#[test]
fn geometry_expansion_examples() {
    check_rejection(
        serde_json::json!({"type":"line","from":[0,0],"to":[0,0]}),
        ErrorKind::Shape,
        "shapes[0].to",
    );
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/layout-sketch/assets/examples"
    );
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 6);
    for path in paths {
        let source = parse_scene(&std::fs::read(&path).unwrap()).unwrap();
        let first = expand_scene(&source).unwrap();
        assert_eq!(first, expand_scene(&source).unwrap());
        for item in &first {
            assert!(
                item.geometry
                    .bounds
                    .min
                    .iter()
                    .chain(item.geometry.bounds.max.iter())
                    .all(|v| v.is_finite())
            );
            for segment in &item.geometry.segments {
                let points: Vec<Point> = match segment {
                    Segment::Move(a) | Segment::Line(a) => vec![*a],
                    Segment::Quadratic(a, b) => vec![*a, *b],
                    Segment::Cubic(a, b, c) => vec![*a, *b, *c],
                    Segment::Close => vec![],
                };
                assert!(points.iter().flatten().all(|v| v.is_finite()));
            }
        }
        if path.file_name().unwrap() == "04-group-repeat.json" {
            let mut expected = vec![("rim", "shapes[0].shapes[0]", None)];
            for index in 0..6 {
                expected.push((
                    "spoke-segment",
                    "shapes[0].shapes[1].shapes[0]",
                    Some(index),
                ));
            }
            expected.push(("hub", "shapes[0].shapes[2]", None));
            for index in 0..8 {
                expected.push(("ray", "shapes[1].shapes[0]", Some(index)));
            }
            expected.push(("sun-center", "shapes[2]", None));
            assert_eq!(first.len(), expected.len());
            for (item, (id, path, index)) in first.iter().zip(expected) {
                assert_eq!(item.geometry.id.as_deref(), Some(id));
                assert_eq!(item.geometry.source_path, path);
                assert_eq!(item.repeat_index, index);
                assert!(item.allow_clip.is_empty());
            }
        }
    }
}

fn parse_node(node: serde_json::Value) -> Result<crate::scene::Scene, SceneError> {
    parse_scene(
        serde_json::json!({"schema_version":1,"shapes":[node]})
            .to_string()
            .as_bytes(),
    )
}
fn check_rejection(node: serde_json::Value, kind: ErrorKind, path: &str) {
    let failure = parse_node(node).unwrap_err();
    assert_eq!(failure.kind, kind, "{failure}");
    assert_eq!(failure.path, path, "{failure}");
}
fn line() -> serde_json::Value {
    serde_json::json!({"type":"line","from":[0,0],"to":[1,0]})
}
fn check_group_cases() {
    let cases = [
        (
            serde_json::json!({"flip_x":true}),
            [-10.24, 5.12],
            [-20.48, 5.12],
        ),
        (
            serde_json::json!({"scale":0.5}),
            [5.12, 2.56],
            [10.24, 2.56],
        ),
        (
            serde_json::json!({"rotate":90}),
            [-5.12, 10.24],
            [-5.12, 20.48],
        ),
        (
            serde_json::json!({"at":[-2,3]}),
            [-10.24, 35.84],
            [0.0, 35.84],
        ),
        (
            serde_json::json!({"rotate":45}),
            [3.620386719675123, 10.86116015902537],
            [10.86116015902537, 18.101933598375617],
        ),
    ];
    for (properties, from, to) in cases {
        let mut node = serde_json::json!({"type":"group","shapes":[{"type":"line","from":[1,0.5],"to":[2,0.5]}]});
        node.as_object_mut()
            .unwrap()
            .extend(properties.as_object().unwrap().clone());
        endpoints(&expand(node)[0], from, to);
    }
    let items = expand(
        serde_json::json!({"type":"group","scale":2,"rotate":90,"at":[10,10],"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}),
    );
    endpoints(&items[0], [102.4, 102.4], [102.4, 143.36]);
    for scale in [0, -1] {
        check_rejection(
            serde_json::json!({"type":"group","scale":scale,"shapes":[line()]}),
            ErrorKind::Shape,
            "shapes[0].scale",
        );
    }
    check_rejection(
        serde_json::json!({"type":"group","shapes":[{"type":"group","shapes":[line()]}]}),
        ErrorKind::Structure,
        "shapes[0].shapes[0].type",
    );
}
fn check_repeat_cases() {
    for (count, start, step) in [
        (1, 90, 90),
        (2, 0, 90),
        (2, 0, -90),
        (2, 90, 0),
        (256, 0, 0),
    ] {
        let items = expand(
            serde_json::json!({"type":"repeat_rotate","center":[-3,2],"count":count,"start":start,"step":step,"shapes":[{"type":"line","from":[1,0],"to":[2,0]}]}),
        );
        assert_eq!(items.len(), count);
        for (index, item) in items.iter().enumerate() {
            let angle = ((start + index as i32 * step) as f64).to_radians();
            endpoints(
                item,
                [(-3.0 + angle.cos()) * 10.24, (2.0 + angle.sin()) * 10.24],
                [
                    (-3.0 + 2.0 * angle.cos()) * 10.24,
                    (2.0 + 2.0 * angle.sin()) * 10.24,
                ],
            );
            assert_eq!(item.repeat_index, Some(index as u16));
        }
    }
    let full = expand(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":256,"shapes":[line()]}),
    );
    assert_eq!(full.len(), 256);
    for (index, item) in full.iter().enumerate() {
        let angle = index as f64 * std::f64::consts::TAU / 256.0;
        endpoints(item, [0.0, 0.0], [10.24 * angle.cos(), 10.24 * angle.sin()]);
    }
    for count in [
        serde_json::json!(0),
        serde_json::json!(257),
        serde_json::json!(1.5),
    ] {
        check_rejection(
            serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":count,"shapes":[line()]}),
            ErrorKind::Structure,
            "shapes[0].count",
        );
    }
    check_rejection(
        serde_json::json!({"type":"repeat_rotate","count":1,"shapes":[line()]}),
        ErrorKind::Structure,
        "shapes[0].center",
    );
    check_rejection(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"shapes":[line()]}),
        ErrorKind::Structure,
        "shapes[0].count",
    );
    check_rejection(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[line()]}]}),
        ErrorKind::Structure,
        "shapes[0].shapes[0].type",
    );
}
fn check_limit_cases() {
    let child = line();
    for count in [10000, 10001] {
        let text =
            serde_json::json!({"schema_version":1,"shapes":vec![child.clone();count]}).to_string();
        let parsed = parse_scene(text.as_bytes());
        if count == 10000 {
            assert_eq!(expand_scene(&parsed.unwrap()).unwrap().len(), count);
        } else {
            let e = parsed.unwrap_err();
            assert_eq!((e.kind, e.path.as_str()), (ErrorKind::Limit, "shapes"));
            assert!(e.detail.contains("10000"));
        }
    }
    for extra in [false, true] {
        let mut group_children = vec![child.clone(); 99];
        group_children.push(serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":100,"shapes":vec![child.clone();99]}));
        let mut nodes = vec![
            child.clone(),
            serde_json::json!({"type":"group","shapes":group_children}),
        ];
        if extra {
            nodes.push(child.clone());
        }
        let text = serde_json::json!({"schema_version":1,"shapes":nodes}).to_string();
        let parsed = parse_scene(text.as_bytes());
        if extra {
            let e = parsed.unwrap_err();
            assert_eq!((e.kind, e.path.as_str()), (ErrorKind::Limit, "shapes"));
            assert!(e.detail.contains("10000"));
        } else {
            let mut s = parsed.unwrap();
            assert_eq!(expand_scene(&s).unwrap().len(), 10000);
            let extra_node = parse_node(child.clone()).unwrap().shapes.remove(0);
            s.shapes.push(extra_node);
            let e = expand_scene(&s).unwrap_err();
            assert_eq!((e.kind, e.path.as_str()), (ErrorKind::Limit, "shapes"));
        }
    }
}
fn check_finite_cases() {
    let cases = [
        (
            serde_json::json!({"type":"group","scale":1e307,"shapes":[{"type":"line","from":[0,0],"to":[100,0]}]}),
            "shapes[0].shapes[0]",
            None,
        ),
        (
            serde_json::json!({"type":"group","at":[1e308,0],"scale":1e307,"shapes":[{"type":"line","from":[0,0],"to":[10,0]}]}),
            "shapes[0].shapes[0]",
            None,
        ),
        (
            serde_json::json!({"type":"repeat_rotate","center":[1.75e308,0],"count":1,"shapes":[{"type":"line","from":[1e307,0],"to":[1.1e307,0]}]}),
            "shapes[0].shapes[0]",
            Some(0),
        ),
        (
            serde_json::json!({"type":"group","at":[1.76e307,0],"shapes":[{"type":"line","from":[0,0],"to":[1,0]}]}),
            "shapes[0].shapes[0]",
            None,
        ),
        (
            serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":3,"start":0,"step":1e308,"shapes":[line()]}),
            "shapes[0].step",
            Some(2),
        ),
        (
            serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":2,"start":1e308,"step":1e308,"shapes":[line()]}),
            "shapes[0].start",
            Some(1),
        ),
    ];
    for (node, path, index) in cases {
        let failure = expand_scene(&parse_node(node).unwrap()).unwrap_err();
        assert_eq!(failure.kind, ErrorKind::Shape);
        assert_eq!(failure.path, path);
        let detail = match path {
            "shapes[0].step" => "repeat angle product is not finite",
            "shapes[0].start" => "repeat angle sum is not finite",
            _ => "transformed coordinate is not finite",
        };
        let expected = match index {
            Some(i) => format!("{detail}; repeat_index={i}"),
            None => detail.to_owned(),
        };
        assert_eq!(failure.detail, expected);
    }
    let item = expand(
        serde_json::json!({"type":"group","rotate":1e300,"at":[-1000,1000],"shapes":[line()]}),
    );
    // The decoded binary64 value 1e300 is an exact integer divisible by 360.
    endpoints(&item[0], [-10240.0, 10240.0], [-10229.76, 10240.0]);
}

fn check_order_cases() {
    let input = serde_json::json!({"schema_version":1,"shapes":[
        {"id":"A","type":"line","from":[0,0],"to":[1,0],"allow_clip":["left"]},
        {"type":"group","allow_clip":["top"],"shapes":[
            {"id":"B","type":"line","from":[0,0],"to":[1,0]},
            {"type":"repeat_rotate","center":[0,0],"count":2,"allow_clip":["right"],"shapes":[
                {"id":"C","type":"line","from":[0,0],"to":[1,0],"allow_clip":["top","bottom"]},
                {"type":"line","from":[0,0],"to":[2,0]}
            ]},
            {"id":"E","type":"line","from":[0,0],"to":[1,0]}
        ]},
        {"id":"F","type":"line","from":[0,0],"to":[1,0]}
    ]})
    .to_string();
    let source = parse_scene(input.as_bytes()).unwrap();
    let items = expand_scene(&source).unwrap();
    assert_eq!(items, expand_scene(&source).unwrap());
    assert_eq!(source, parse_scene(input.as_bytes()).unwrap());
    let expected = [
        (Some("A"), "shapes[0]", None, vec![ClipSide::Left]),
        (Some("B"), "shapes[1].shapes[0]", None, vec![ClipSide::Top]),
        (
            Some("C"),
            "shapes[1].shapes[1].shapes[0]",
            Some(0),
            vec![ClipSide::Top, ClipSide::Right, ClipSide::Bottom],
        ),
        (
            None,
            "shapes[1].shapes[1].shapes[1]",
            Some(0),
            vec![ClipSide::Top, ClipSide::Right],
        ),
        (
            Some("C"),
            "shapes[1].shapes[1].shapes[0]",
            Some(1),
            vec![ClipSide::Top, ClipSide::Right, ClipSide::Bottom],
        ),
        (
            None,
            "shapes[1].shapes[1].shapes[1]",
            Some(1),
            vec![ClipSide::Top, ClipSide::Right],
        ),
        (Some("E"), "shapes[1].shapes[2]", None, vec![ClipSide::Top]),
        (Some("F"), "shapes[2]", None, vec![]),
    ];
    assert_eq!(items.len(), expected.len());
    let end_points = [
        [10.24, 0.0],
        [10.24, 0.0],
        [10.24, 0.0],
        [20.48, 0.0],
        [-10.24, 0.0],
        [-20.48, 0.0],
        [10.24, 0.0],
        [10.24, 0.0],
    ];
    for (item, end) in items.iter().zip(end_points) {
        endpoints(item, [0.0, 0.0], end);
    }
    for (item, (id, path, index, clips)) in items.iter().zip(expected) {
        assert_eq!(item.geometry.id.as_deref(), id);
        assert_eq!(item.geometry.source_path, path);
        assert_eq!(item.repeat_index, index);
        assert_clip_sides(&item.allow_clip, &clips);
    }
    let items = expand(
        serde_json::json!({"type":"group","scale":2,"rotate":90,"at":[10,10],"shapes":[{"type":"repeat_rotate","center":[3,4],"count":2,"step":90,"shapes":[{"type":"line","from":[0,0],"to":[2,0]}]}]}),
    );
    endpoints(&items[0], [20.48, 163.84], [20.48, 204.8]);
    endpoints(&items[1], [20.48, 163.84], [-20.48, 163.84]);
}
fn check_style_cases() {
    for width in ["thin", "thick"] {
        for fill in ["none", "white", "black"] {
            for scale in [1, 2] {
                let rect = serde_json::json!({"type":"rect","x":0,"y":0,"w":2,"h":1,"width":width,"fill":fill});
                let node = serde_json::json!({"type":"group","scale":scale,"flip_x":true,"shapes":[rect.clone(),{"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[rect]}]});
                let source = scene(node.clone());
                let items = expand_scene(&source).unwrap();
                assert_eq!(source, scene(node));
                for item in items {
                    assert_eq!(
                        item.geometry.width,
                        if width == "thin" {
                            crate::scene::Width::Thin
                        } else {
                            crate::scene::Width::Thick
                        }
                    );
                    assert_eq!(
                        item.geometry.fill,
                        match fill {
                            "white" => Fill::White,
                            "black" => Fill::Black,
                            _ => Fill::None,
                        }
                    );
                    assert_eq!(item.geometry.fill_rule, tiny_skia::FillRule::EvenOdd);
                    near(
                        item.geometry.bounds.max[0] - item.geometry.bounds.min[0],
                        20.48 * scale as f64,
                    );
                }
            }
        }
    }
    for field in ["width", "fill"] {
        let mut node = serde_json::json!({"type":"group","shapes":[line()]});
        node[field] = serde_json::json!(if field == "width" { "thin" } else { "white" });
        check_rejection(node, ErrorKind::Structure, &format!("shapes[0].{field}"));
        let mut repeat =
            serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[line()]});
        repeat[field] = serde_json::json!(if field == "width" { "thin" } else { "white" });
        check_rejection(repeat, ErrorKind::Structure, &format!("shapes[0].{field}"));
    }
    check_rejection(
        serde_json::json!({"type":"line","from":[0,0],"to":[1,0],"fill":"white"}),
        ErrorKind::Shape,
        "shapes[0].fill",
    );
}
fn check_path_cases() {
    for canvas in ["landscape", "portrait", "square"] {
        let text=serde_json::json!({"schema_version":1,"canvas":canvas,"shapes":[{"type":"line","from":[1,1],"to":[2,1]}]}).to_string();
        let items = expand_scene(&parse_scene(text.as_bytes()).unwrap()).unwrap();
        endpoints(&items[0], [10.24, 10.24], [20.48, 10.24]);
    }
    let cubic = expand(serde_json::json!({"type":"path","d":"M0 0C0 2 2 2 2 0"}));
    assert_point(cubic[0].geometry.bounds.max, [20.48, 15.36]);
    let rotated = expand(
        serde_json::json!({"type":"group","rotate":45,"shapes":[{"type":"path","d":"M0 0C0 1 0 1 0 0"}]}),
    );
    let height = 0.75 * 10.24 / std::f64::consts::SQRT_2;
    assert_point(rotated[0].geometry.bounds.min, [-height, 0.0]);
    assert_point(rotated[0].geometry.bounds.max, [0.0, height]);
    let closed = expand(
        serde_json::json!({"type":"group","flip_x":true,"shapes":[{"type":"path","d":"M0 0L1 0L1 1ZM2 0L3 0L3 1Z","fill":"black"}]}),
    );
    assert_eq!(closed[0].geometry.fill_rule, tiny_skia::FillRule::EvenOdd);
    assert_eq!(
        closed[0]
            .geometry
            .segments
            .iter()
            .filter(|s| matches!(s, Segment::Move(_)))
            .count(),
        2
    );
    assert_eq!(
        closed[0]
            .geometry
            .segments
            .iter()
            .filter(|s| matches!(s, Segment::Close))
            .count(),
        2
    );
    let cases = [
        serde_json::json!({"type":"line","from":[0,0],"to":[2,0]}),
        serde_json::json!({"type":"polyline","pts":[[0,0],[1,1],[2,0]]}),
        serde_json::json!({"type":"polygon","pts":[[0,0],[1,1],[2,0]]}),
        serde_json::json!({"type":"rect","x":0,"y":0,"w":2,"h":1}),
        serde_json::json!({"type":"curve","from":[0,0],"ctrl1":[0,1],"ctrl2":[1,1],"to":[2,0]}),
        serde_json::json!({"type":"path","d":"M0 0Q1 2 2 0M2 0C2 1 3 1 3 0Z"}),
        serde_json::json!({"type":"circle","cx":0,"cy":0,"r":1}),
        serde_json::json!({"type":"ellipse","cx":0,"cy":0,"rx":1,"ry":2}),
        serde_json::json!({"type":"arc","cx":0,"cy":0,"rx":1,"ry":2,"start":0,"sweep":90}),
    ];
    for node in cases {
        let source = scene(node.clone());
        let Shape::Primitive { primitive, style } = &source.shapes[0].shape else {
            panic!()
        };
        let local = build_primitive(primitive, style, "shapes[0]", None, 10.24).unwrap();
        let result = expand(serde_json::json!({"type":"group","flip_x":true,"shapes":[node]}));
        assert_eq!(local.segments.len(), result[0].geometry.segments.len());
        for (a, b) in local.segments.iter().zip(&result[0].geometry.segments) {
            let reflect = |p: Point| [-p[0] * 10.24, p[1] * 10.24];
            match (a, b) {
                (Segment::Move(x), Segment::Move(y)) | (Segment::Line(x), Segment::Line(y)) => {
                    assert_point(*y, reflect(*x))
                }
                (Segment::Quadratic(a, b), Segment::Quadratic(c, d)) => {
                    assert_point(*c, reflect(*a));
                    assert_point(*d, reflect(*b));
                }
                (Segment::Cubic(a, b, c), Segment::Cubic(d, e, f)) => {
                    assert_point(*d, reflect(*a));
                    assert_point(*e, reflect(*b));
                    assert_point(*f, reflect(*c));
                }
                (Segment::Close, Segment::Close) => {}
                _ => panic!("segment mismatch"),
            }
        }
    }
    for chord in [false, true] {
        let arc = expand(
            serde_json::json!({"type":"arc","cx":0,"cy":0,"rx":1,"ry":2,"start":0,"sweep":90,"chord":chord}),
        );
        assert_eq!(
            matches!(arc[0].geometry.segments.last(), Some(Segment::Close)),
            chord
        );
    }
}

fn check_accuracy_cases() {
    for scale in [1000.0, 0.5] {
        for repeated in [false, true] {
            for kind in 0..4 {
                let (rx, ry, ellipse_angle, start, sweep, close) = match kind {
                    0 => (2.0, 2.0, 0.0_f64, 0.0, 360.0, true),
                    1 => (3.0, 1.0, 25.0, 0.0, 360.0, true),
                    2 => (3.0, 1.0, 25.0, 20.0, 220.0, false),
                    _ => (3.0, 1.0, 25.0, 20.0, -220.0, true),
                };
                let shape = match kind {
                    0 => serde_json::json!({"type":"circle","cx":1,"cy":-2,"r":rx}),
                    1 => {
                        serde_json::json!({"type":"ellipse","cx":1,"cy":-2,"rx":rx,"ry":ry,"angle":ellipse_angle})
                    }
                    _ => {
                        serde_json::json!({"type":"arc","cx":1,"cy":-2,"rx":rx,"ry":ry,"angle":ellipse_angle,"start":start,"sweep":sweep,"chord":close})
                    }
                };
                let inner = if repeated {
                    serde_json::json!({"type":"repeat_rotate","center":[3,4],"count":1,"start":45,"shapes":[shape]})
                } else {
                    shape
                };
                let items = expand(
                    serde_json::json!({"type":"group","scale":scale,"rotate":30,"flip_x":true,"at":[10,-5],"shapes":[inner]}),
                );
                let segments = &items[0].geometry.segments;
                assert_eq!(matches!(segments.last(), Some(Segment::Close)), close);
                let lines = segments.len() - 1 - usize::from(close);
                let vertex = |i| match segments[i] {
                    Segment::Move(p) | Segment::Line(p) => p,
                    _ => panic!(),
                };
                for sample in 0..=4096 {
                    let t = sample as f64 / 4096.0;
                    let i = ((t * lines as f64) as usize).min(lines - 1);
                    let f = t * lines as f64 - i as f64;
                    let a = vertex(i);
                    let b = vertex(i + 1);
                    let parameter = (start + sweep * t).to_radians();
                    let e = ellipse_angle.to_radians();
                    let mut x =
                        1.0 + rx * parameter.cos() * e.cos() - ry * parameter.sin() * e.sin();
                    let mut y =
                        -2.0 + rx * parameter.cos() * e.sin() + ry * parameter.sin() * e.cos();
                    if repeated {
                        let q = 45f64.to_radians();
                        let px = x * q.cos() - y * q.sin() + 3.0;
                        y = x * q.sin() + y * q.cos() + 4.0;
                        x = px;
                    }
                    x = -x * scale;
                    y *= scale;
                    let q = 30f64.to_radians();
                    let expected = [
                        (x * q.cos() - y * q.sin() + 10.0) * 10.24,
                        (x * q.sin() + y * q.cos() - 5.0) * 10.24,
                    ];
                    let actual = [a[0] * (1.0 - f) + b[0] * f, a[1] * (1.0 - f) + b[1] * f];
                    assert!(
                        (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 0.2500001,
                        "kind={kind}, scale={scale}, repeated={repeated}, t={t}"
                    );
                    if sample == 0 || sample == 4096 {
                        assert_point(actual, expected);
                    }
                }
            }
        }
    }
    let source = scene(
        serde_json::json!({"type":"group","at":[1e16,0],"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}),
    );
    let error = expand_scene(&source).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Shape);
    assert_eq!(error.path, "shapes[0].shapes[0]");
    assert_eq!(
        error.detail,
        "ellipse transform accuracy cannot be represented"
    );
    let repeated = scene(
        serde_json::json!({"type":"group","at":[1e16,0],"shapes":[{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}]}),
    );
    let error = expand_scene(&repeated).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Shape);
    assert_eq!(error.path, "shapes[0].shapes[0].shapes[0]");
    assert_eq!(
        error.detail,
        "ellipse transform accuracy cannot be represented; repeat_index=0"
    );
}

#[test]
fn geometry_expansion_refactoring_pin() {
    let source = scene(
        serde_json::json!({"type":"group","at":[10,10],"rotate":90,"scale":2,"flip_x":true,"allow_clip":["top"],"shapes":[{"type":"repeat_rotate","center":[3,4],"count":2,"step":90,"allow_clip":["left"],"shapes":[{"id":"child","type":"line","from":[0,0],"to":[2,0],"width":"thin"}]}]}),
    );
    let items = expand_scene(&source).unwrap();
    assert_eq!(items.len(), 2);
    endpoints(&items[0], [20.48, 40.96], [20.48, 0.0]);
    endpoints(&items[1], [20.48, 40.96], [-20.48, 40.96]);
    for (index, item) in items.iter().enumerate() {
        assert_eq!(item.geometry.source_path, "shapes[0].shapes[0].shapes[0]");
        assert_eq!(item.geometry.id.as_deref(), Some("child"));
        assert_eq!(item.repeat_index, Some(index as u16));
        assert_clip_sides(&item.allow_clip, &[ClipSide::Top, ClipSide::Left]);
        assert_eq!(item.geometry.width, crate::scene::Width::Thin);
    }
    let source = scene(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":3,"step":1e308,"shapes":[{"type":"line","from":[0,0],"to":[1,0]}]}),
    );
    let failure = expand_scene(&source).unwrap_err();
    assert_eq!(failure.kind, ErrorKind::Shape);
    assert_eq!(failure.path, "shapes[0].step");
    assert_eq!(
        failure.detail,
        "repeat angle product is not finite; repeat_index=2"
    );
}

fn assert_clip_sides(actual: &[ClipSide], expected: &[ClipSide]) {
    assert_eq!(actual.len(), expected.len());
    for (index, side) in actual.iter().enumerate() {
        assert!(expected.contains(side));
        assert!(!actual[..index].contains(side));
    }
}

fn check_repeat_phase_accuracy() {
    // Exact integer arithmetic on the decoded binary64 inputs, before rounding.
    let sum_phase = (10_000_000_000_000_000u64 + 1) % 360;
    let product_phase = (3u64 * 10_000_000_000_000_002u64) % 360;
    assert_eq!((sum_phase, product_phase), (281, 126));
    let rounded_sum = (1e16_f64 + 1.0).rem_euclid(360.0);
    let rounded_product = (3.0_f64 * 10000000000000002.0).rem_euclid(360.0);
    for (exact, rounded) in [
        (sum_phase as f64, rounded_sum),
        (product_phase as f64, rounded_product),
    ] {
        let delta = (exact - rounded).to_radians();
        let displacement = 2.0 * 50.0 * 10.24 * (delta / 2.0).sin().abs();
        assert!(displacement > 0.25);
    }
    let small = expand(
        serde_json::json!({"type":"repeat_rotate","center":[0,0],"count":2,"start":1e16,"step":1,"shapes":[{"type":"circle","cx":0,"cy":0,"r":0.001}]}),
    );
    let Segment::Move(point) = small[1].geometry.segments[0] else {
        panic!()
    };
    let phase = (sum_phase as f64).to_radians();
    let exact = [0.001 * 10.24 * phase.cos(), 0.001 * 10.24 * phase.sin()];
    assert!((point[0] - exact[0]).hypot(point[1] - exact[1]) < 0.25);
    let tiny = expand(
        serde_json::json!({"type":"group","scale":1e-200,"shapes":[{"type":"repeat_rotate","center":[0,0],"count":3,"shapes":[{"type":"circle","cx":0,"cy":0,"r":1}]}]}),
    );
    assert_eq!(tiny.len(), 3);

    for nested in [false, true] {
        for (start, step, count, index) in [(1e16, 1.0, 2, 1), (0.0, 10000000000000002.0, 4, 3)] {
            let repeat = serde_json::json!({"type":"repeat_rotate","center":[100,0],"count":count,"start":start,"step":step,"shapes":[{"type":"circle","cx":0,"cy":0,"r":50}]});
            let node = if nested {
                serde_json::json!({"type":"group","rotate":30,"scale":2,"shapes":[repeat]})
            } else {
                repeat
            };
            let failure = expand_scene(&scene(node)).unwrap_err();
            assert_eq!(failure.kind, ErrorKind::Shape);
            let path = if nested {
                "shapes[0].shapes[0].shapes[0]"
            } else {
                "shapes[0].shapes[0]"
            };
            assert_eq!(failure.path, path);
            assert_eq!(
                failure.detail,
                format!("ellipse transform accuracy cannot be represented; repeat_index={index}")
            );
        }
    }
}
