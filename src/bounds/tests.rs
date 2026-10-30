use super::*;
use crate::geometry::{ExpandedPrimitive, expand_scene};
use crate::path_data::Segment;
use crate::scene::{ErrorKind, Scene, parse_scene};
use serde_json::{Value, json};

fn scene(canvas: &str, shapes: Vec<Value>) -> Scene {
    let input = json!({"schema_version":1,"canvas":canvas,"shapes":shapes}).to_string();
    parse_scene(input.as_bytes()).unwrap()
}
fn line(from: [f64; 2], to: [f64; 2], width: &str) -> Value {
    json!({"type":"line","from":from,"to":to,"width":width})
}
fn warnings(canvas: &str, shapes: Vec<Value>) -> Vec<OutsideWarning> {
    let source = scene(canvas, shapes);
    check_bounds(source.canvas, &expand_scene(&source).unwrap()).unwrap()
}
fn sides(warnings: &[OutsideWarning]) -> Vec<ClipSide> {
    warnings.iter().map(|warning| warning.edge).collect()
}
fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} vs {expected}"
    );
}
#[test]
fn bounds_edges() {
    assert!(!exceeds_tolerance(0.1));
    assert!(exceeds_tolerance(0.100000001));
    assert!(check_bounds(Canvas::Square, &[]).unwrap().is_empty());
    assert!(parse_scene(br#"{"schema_version":1,"shapes":[]}"#).is_err());
    for (width, excess) in [("thick", 4.5), ("thin", 2.25)] {
        let out = warnings("square", vec![line([0.0, 20.0], [0.0, 80.0], width)]);
        assert_eq!(sides(&out), [ClipSide::Left]);
        near(out[0].excess_px, excess, 0.1);
    }
    for (canvas, end) in [
        ("landscape", [150.0, 100.0]),
        ("portrait", [100.0, 150.0]),
        ("square", [100.0, 100.0]),
    ] {
        let out = warnings(canvas, vec![line([0.0, 0.0], end, "thick")]);
        assert_eq!(
            sides(&out),
            [
                ClipSide::Left,
                ClipSide::Top,
                ClipSide::Right,
                ClipSide::Bottom
            ]
        );
        for warning in out {
            near(warning.excess_px, 4.5, 0.1);
        }
    }
    for (x, expected) in [(4.5, false), (5.0, false), (4.45, false), (4.35, true)] {
        let out = warnings(
            "square",
            vec![line([x / 10.24, 20.0], [x / 10.24, 80.0], "thick")],
        );
        assert_eq!(!out.is_empty(), expected);
        if expected {
            assert_eq!(sides(&out), [ClipSide::Left]);
            near(out[0].excess_px, 0.15, 0.01);
        }
    }
    let out = warnings("square", vec![line([0.2, 30.0], [10.0, 30.0], "thick")]);
    assert_eq!(sides(&out), [ClipSide::Left]);
    near(out[0].excess_px, 4.5 - 2.048, 0.1);
}
#[test]
fn bounds_path_extrema() {
    let safe = warnings(
        "square",
        vec![json!({"type":"path","d":"M10 10Q-5 20 10 30"})],
    );
    assert!(safe.is_empty());
    let crossed = warnings(
        "square",
        vec![json!({"type":"path","d":"M10 10Q-15 20 10 30"})],
    );
    assert_eq!(sides(&crossed), [ClipSide::Left]);
    near(crossed[0].excess_px, 30.1, 0.35);
    let cubic = warnings(
        "square",
        vec![json!({"type":"path","d":"M20 50C30 250 40 -150 50 50"})],
    );
    assert_eq!(sides(&cubic), [ClipSide::Top, ClipSide::Bottom]);
    for item in cubic {
        near(
            item.excess_px,
            (100.0 / 3.0_f64.sqrt() - 50.0) * 10.24 + 4.5,
            0.35,
        );
    }
    let safe = json!({"type":"path","d":"M10 10C-1 20 -1 30 10 40"});
    assert!(warnings("square", vec![safe.clone()]).is_empty());
    assert!(
        warnings(
            "square",
            vec![json!({"type":"group","at":[100,0],"flip_x":true,"shapes":[safe]})]
        )
        .is_empty()
    );
    for fill in ["white", "black"] {
        let out = warnings(
            "square",
            vec![json!({"type":"path","d":"M0 10L10 10L10 20Z","fill":fill})],
        );
        assert_eq!(sides(&out), [ClipSide::Left]);
        near(out[0].excess_px, 4.5, 0.1);
    }
    let polygon = warnings(
        "square",
        vec![json!({"type":"polygon","pts":[[0,10],[10,10],[10,20]]})],
    );
    assert_eq!(sides(&polygon), [ClipSide::Left]);
    let arc = warnings(
        "square",
        vec![json!({"type":"arc","cx":0,"cy":30,"rx":2,"ry":2,"start":0,"sweep":180,"chord":true})],
    );
    assert_eq!(sides(&arc), [ClipSide::Left]);
    let open = warnings(
        "square",
        vec![json!({"type":"path","d":"M10 10L-1 20L10 30 M10 40L-1 50L10 60"})],
    );
    assert_eq!(sides(&open), [ClipSide::Left]);
}
#[test]
fn bounds_transforms() {
    let ellipse = |angle| json!({"type":"ellipse","cx":96,"cy":50,"rx":2,"ry":8,"angle":angle});
    assert!(warnings("square", vec![ellipse(0)]).is_empty());
    let out = warnings("square", vec![ellipse(45)]);
    assert_eq!(sides(&out), [ClipSide::Right]);
    near(
        out[0].excess_px,
        (96.0 + 34.0_f64.sqrt() - 100.0) * 10.24 + 4.5,
        0.6,
    );
    for (width, excess) in [("thick", 4.5), ("thin", 2.25)] {
        for scale in [1, 2] {
            for flip in [false, true] {
                let out = warnings(
                    "square",
                    vec![
                        json!({"type":"group","at":[0,50],"rotate":90,"scale":scale,"flip_x":flip,"shapes":[line([0.0,0.0],[10.0,0.0],width)]}),
                    ],
                );
                assert_eq!(sides(&out), [ClipSide::Left]);
                near(out[0].excess_px, excess, 0.1);
            }
        }
    }
    let out = warnings(
        "square",
        vec![
            json!({"type":"group","shapes":[{"type":"repeat_rotate","center":[90,50],"count":4,"step":90,"shapes":[{"type":"line","id":"ray","from":[0,0],"to":[15,0]}]}]}),
        ],
    );
    assert_eq!(sides(&out), [ClipSide::Right]);
    near(out[0].excess_px, 55.7, 0.1);
    assert_eq!(out[0].source_path, "shapes[0].shapes[0].shapes[0]");
    assert_eq!(out[0].id.as_deref(), Some("ray"));
    assert_eq!(out[0].repeat_index, Some(0));
}

#[test]
fn bounds_allow_clip() {
    let corner = json!({"type":"line","from":[0,0],"to":[20,20]});
    let out = warnings("square", vec![corner.clone()]);
    assert_eq!(sides(&out), [ClipSide::Left, ClipSide::Top]);
    let allowed = json!({"type":"line","from":[0,0],"to":[20,20],"allow_clip":["left"]});
    assert_eq!(sides(&warnings("square", vec![allowed])), [ClipSide::Top]);
    let node = json!({"type":"group","flip_x":true,"rotate":180,"at":[0,100],"allow_clip":["left"],"shapes":[{"type":"repeat_rotate","center":[0,0],"count":1,"allow_clip":["top","left"],"shapes":[{"type":"rect","x":-10,"y":-10,"w":120,"h":120,"allow_clip":["right"]}]}]});
    let input=json!({"schema_version":1,"canvas":"square","shapes":[node,{"type":"line","from":[0,40],"to":[0,60]}]}).to_string();
    let source = parse_scene(input.as_bytes()).unwrap();
    let expanded = expand_scene(&source).unwrap();
    let before = expand_scene(&source).unwrap();
    let result = check_bounds(source.canvas, &expanded).unwrap();
    assert_eq!(sides(&result), [ClipSide::Bottom, ClipSide::Left]);
    assert_eq!(result[0].source_path, "shapes[0].shapes[0].shapes[0]");
    assert_eq!(result[1].source_path, "shapes[1]");
    assert_eq!(source, parse_scene(input.as_bytes()).unwrap());
    assert_eq!(expanded, before);
    let inherited = warnings(
        "square",
        vec![
            json!({"type":"group","allow_clip":["left"],"shapes":[{"type":"repeat_rotate","center":[0,0],"count":1,"shapes":[{"type":"line","from":[0,0],"to":[20,20],"allow_clip":[]}]}]}),
        ],
    );
    assert_eq!(sides(&inherited), [ClipSide::Top]);
    let reordered = warnings(
        "square",
        vec![json!({"type":"line","from":[0,0],"to":[100,100],"allow_clip":["bottom","top"]})],
    );
    let ordered = warnings(
        "square",
        vec![json!({"type":"line","from":[0,0],"to":[100,100],"allow_clip":["top","bottom"]})],
    );
    assert_eq!(reordered, ordered);
    assert_eq!(sides(&ordered), [ClipSide::Left, ClipSide::Right]);
    let original = scene("square", vec![corner]);
    let allowed = scene(
        "square",
        vec![
            json!({"type":"line","from":[0,0],"to":[20,20],"allow_clip":["bottom","right","top","left"]}),
        ],
    );
    let a = expand_scene(&original).unwrap();
    let b = expand_scene(&allowed).unwrap();
    assert!(check_bounds(allowed.canvas, &b).unwrap().is_empty());
    assert_eq!(
        render::render_main(original.canvas, &a).unwrap(),
        render::render_main(allowed.canvas, &b).unwrap()
    );
    let mut invalid = b;
    invalid[0].geometry.segments[0] = Segment::Move([f64::NAN, 0.0]);
    assert!(matches!(
        check_bounds(Canvas::Square, &invalid),
        Err(RenderError::InvalidGeometry(_))
    ));
}
#[test]
fn bounds_diagnostics() {
    let out = warnings(
        "square",
        vec![json!({"type":"line","id":"axis","from":[0,20],"to":[0,80]})],
    );
    assert_eq!(
        out[0].to_string(),
        "WARN OUTSIDE path=shapes[0] id=axis edge=left excess_px=4.5"
    );
    let a = json!({"type":"line","id":"z","from":[0,0],"to":[100,100]});
    let b = json!({"type":"group","shapes":[{"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[{"type":"line","id":"a","from":[0,0],"to":[100,100]}]}]});
    let c = json!({"type":"line","from":[0,0],"to":[100,100]});
    let out = warnings("square", vec![a.clone(), b.clone(), c.clone()]);
    assert_eq!(out.len(), 16);
    let mut expected = Vec::new();
    for (path, id, index) in [
        ("shapes[0]", Some("z"), None),
        ("shapes[1].shapes[0].shapes[0]", Some("a"), Some(0)),
        ("shapes[1].shapes[0].shapes[0]", Some("a"), Some(1)),
        ("shapes[2]", None, None),
    ] {
        for edge in [
            ClipSide::Left,
            ClipSide::Top,
            ClipSide::Right,
            ClipSide::Bottom,
        ] {
            expected.push(OutsideWarning {
                source_path: path.into(),
                id: id.map(str::to_owned),
                repeat_index: index,
                edge,
                excess_px: if matches!(edge, ClipSide::Left | ClipSide::Top) {
                    4.499999046325684
                } else {
                    4.5
                },
            });
        }
    }
    assert_eq!(out, expected);
    let expected_strings: Vec<_> = [
        ("shapes[0]", " id=z"),
        ("shapes[1].shapes[0].shapes[0]", " id=a repeat_index=0"),
        ("shapes[1].shapes[0].shapes[0]", " id=a repeat_index=1"),
        ("shapes[2]", ""),
    ]
    .into_iter()
    .flat_map(|(path, metadata)| {
        [
            ("left", "4.499999046325684"),
            ("top", "4.499999046325684"),
            ("right", "4.5"),
            ("bottom", "4.5"),
        ]
        .into_iter()
        .map(move |(edge, excess)| {
            format!("WARN OUTSIDE path={path}{metadata} edge={edge} excess_px={excess}")
        })
    })
    .collect();
    assert_eq!(
        out.iter().map(ToString::to_string).collect::<Vec<_>>(),
        expected_strings
    );
    for actual in &out {
        assert!(actual.excess_px.is_finite() && actual.excess_px > 0.0);
        assert_eq!(actual.to_string().lines().count(), 1);
        assert!(!actual.to_string().contains('\u{1b}'));
        assert!(!actual.to_string().contains('\n'));
        assert!(!actual.to_string().contains("ERROR"));
        assert!(!actual.to_string().contains("OK"));
    }
    let reordered = warnings("square", vec![c, b, a]);
    assert_eq!(reordered[0].id, None);
    assert_eq!(reordered[12].id.as_deref(), Some("z"));
}
fn assert_shape_error(item: &ExpandedPrimitive, expected: &str, detail: &str) {
    let Err(RenderError::InvalidGeometry(e)) =
        check_bounds(Canvas::Square, std::slice::from_ref(item))
    else {
        panic!("expected shape error")
    };
    assert_eq!(e.kind, ErrorKind::Shape);
    assert_eq!(e.path, expected);
    assert_eq!(e.detail, detail);
}
#[test]
fn bounds_failures() {
    for (end, detail) in [
        (1e40, "coordinate cannot be represented as finite f32"),
        (1e37, "coordinate exceeds raster backend numeric range"),
    ] {
        let source = scene("square", vec![line([0.0, 0.0], [end, 0.0], "thick")]);
        let expanded = expand_scene(&source).unwrap();
        assert_shape_error(&expanded[0], "shapes[0]", detail);
    }
    let source = scene(
        "square",
        vec![
            line([0.0, 0.0], [20.0, 20.0], "thick"),
            json!({"type":"repeat_rotate","center":[0,0],"count":2,"allow_clip":["left","top","right","bottom"],"shapes":[{"type":"path","d":"M0 0Q1 2 2 0C2 1 3 1 3 0"}]}),
        ],
    );
    let mut expanded = expand_scene(&source).unwrap();
    expanded[2].geometry.segments[1] = Segment::Quadratic([f64::NAN, 0.0], [20.48, 0.0]);
    assert_shape_error(
        &expanded[2],
        "shapes[1].shapes[0]",
        "coordinate cannot be represented as finite f32; repeat_index=1",
    );
    assert!(matches!(
        check_bounds(source.canvas, &expanded),
        Err(RenderError::InvalidGeometry(_))
    ));
    expanded[2].geometry.segments[1] = Segment::Quadratic([10.24, 20.48], [20.48, 0.0]);
    expanded[2].geometry.segments[2] =
        Segment::Cubic([0.0, 0.0], [0.0, f64::INFINITY], [30.72, 0.0]);
    assert_shape_error(
        &expanded[2],
        "shapes[1].shapes[0]",
        "coordinate cannot be represented as finite f32; repeat_index=1",
    );
    expanded[2].geometry.segments = vec![Segment::Move([0.0, 0.0])];
    let Err(RenderError::Raster {
        source_path,
        repeat_index,
        detail,
    }) = check_bounds(source.canvas, &expanded)
    else {
        panic!("expected raster failure")
    };
    assert_eq!(source_path.as_deref(), Some("shapes[1].shapes[0]"));
    assert_eq!(repeat_index, Some(1));
    assert_eq!(detail, "cannot construct raster path");
    let out = warnings(
        "square",
        vec![
            line([-10.0, 20.0], [10.0, 20.0], "thick"),
            line([20.0, -10.0], [20.0, 10.0], "thick"),
            line([-10.0, -10.0], [-5.0, -10.0], "thin"),
        ],
    );
    assert_eq!(
        sides(&out),
        [ClipSide::Left, ClipSide::Top, ClipSide::Left, ClipSide::Top]
    );
}

#[test]
fn bounds_backend_scan_range() {
    let fixtures = [
        json!({"type":"rect","x":-6e7,"y":-6e7,"w":1.2e8,"h":1.2e8,"fill":"black"}),
        line([0.0, 0.0], [1e9, 1e9 / 3.0], "thick"),
    ];
    for fixture in fixtures {
        let source = scene("square", vec![fixture]);
        let expanded = expand_scene(&source).unwrap();
        assert_shape_error(
            &expanded[0],
            "shapes[0]",
            "coordinate exceeds raster backend numeric range",
        );
        let Err(RenderError::InvalidGeometry(e)) = render::render_main(source.canvas, &expanded)
        else {
            panic!("expected shape error")
        };
        assert_eq!(e.kind, ErrorKind::Shape);
        assert_eq!(e.path, "shapes[0]");
        assert_eq!(e.detail, "coordinate exceeds raster backend numeric range");
    }
    for clipped in [false, true] {
        let mut group = json!({"type":"group","at":[-1e8,0],"shapes":[{"type":"repeat_rotate","center":[0,0],"count":2,"step":180,"start":0,"shapes":[{"type":"line","from":[1e8,20],"to":[1e8+10.0,20]}]}]});
        if clipped {
            group["allow_clip"] = json!(["left", "top", "right", "bottom"]);
        }
        let source = scene("square", vec![group]);
        let expanded = expand_scene(&source).unwrap();
        assert_eq!(expanded.len(), 2);
        check_bounds(source.canvas, &expanded[..1]).unwrap();
        render::render_main(source.canvas, &expanded[..1]).unwrap();
        let detail = "coordinate exceeds raster backend numeric range; repeat_index=1";
        assert_shape_error(&expanded[1], "shapes[0].shapes[0].shapes[0]", detail);
        for error in [
            check_bounds(source.canvas, &expanded).unwrap_err(),
            render::render_main(source.canvas, &expanded).unwrap_err(),
        ] {
            let RenderError::InvalidGeometry(e) = error else {
                panic!("expected shape error")
            };
            assert_eq!(e.kind, ErrorKind::Shape);
            assert_eq!(e.path, "shapes[0].shapes[0].shapes[0]");
            assert_eq!(e.detail, detail);
        }
    }
}

#[test]
fn bounds_pipeline() {
    use image::{DynamicImage, GenericImageView, ImageFormat};
    use std::io::Cursor;

    let crossing = line([-10.0, 20.0], [10.0, 20.0], "thick");
    let source = scene("square", vec![crossing]);
    let expanded = expand_scene(&source).unwrap();
    assert_eq!(
        sides(&check_bounds(source.canvas, &expanded).unwrap()),
        [ClipSide::Left]
    );
    let image = render::render_main(source.canvas, &expanded).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image.clone())
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let decoded = image::load_from_memory(bytes.get_ref()).unwrap();
    assert_eq!(decoded.color(), image::ColorType::Rgb8);
    assert_eq!(decoded.dimensions(), (1024, 1024));
    assert_eq!(decoded.to_rgb8(), image);
    assert!(decoded.to_rgba8().pixels().all(|p| p.0[3] == 255));
    let suppressed = scene(
        "square",
        vec![json!({"type":"line","from":[-10,20],"to":[10,20],"allow_clip":["left"]})],
    );
    let expanded = expand_scene(&suppressed).unwrap();
    assert!(
        check_bounds(suppressed.canvas, &expanded)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        render::render_main(suppressed.canvas, &expanded).unwrap(),
        image
    );
    let covered = scene(
        "square",
        vec![
            line([-10.0, 20.0], [10.0, 20.0], "thick"),
            json!({"type":"rect","x":-1,"y":10,"w":20,"h":20,"fill":"white","allow_clip":["left"]}),
        ],
    );
    let expanded = expand_scene(&covered).unwrap();
    let out = check_bounds(covered.canvas, &expanded).unwrap();
    assert_eq!(sides(&out), [ClipSide::Left]);
    assert_eq!(out[0].source_path, "shapes[0]");
    assert_eq!(
        render::render_main(covered.canvas, &expanded)
            .unwrap()
            .get_pixel(50, 205)
            .0,
        [255, 255, 255]
    );
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/layout-sketch/assets/examples/"
    );
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 6);
    for path in files {
        let input = std::fs::read(path).unwrap();
        let source = parse_scene(&input).unwrap();
        let expanded = expand_scene(&source).unwrap();
        let out = check_bounds(source.canvas, &expanded).unwrap();
        assert_eq!(out, check_bounds(source.canvas, &expanded).unwrap());
        let image = render::render_main(source.canvas, &expanded).unwrap();
        assert_eq!(image.dimensions(), render::canvas_dimensions(source.canvas));
        assert_eq!(source, parse_scene(&input).unwrap());
        assert_eq!(expanded, expand_scene(&source).unwrap());
    }
    let error = parse_scene(
        br#"{"schema_version":1,"shapes":[{"type":"line","from":[0,0],"angle":0,"length":0}]}"#,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Shape);
    assert_eq!(
        error.to_string(),
        "ERROR INVALID_SHAPE path=shapes[0].length: expected a positive number"
    );
    let error = parse_scene(br#"{"schema_version":1,"shapes":[{"type":"path","d":"M0 0 H1"}]}"#)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidPath);
    assert_eq!(error.offset, Some(5));
    assert_eq!(
        error.to_string(),
        "ERROR INVALID_PATH path=shapes[0].d offset=5: unsupported command or trailing input"
    );
}

#[test]
fn bounds_backend_extent_limit() {
    let detail = "path bounds exceed raster precision limit of 16777216 px; reduce the shape size or coordinate magnitudes";
    let m = 536_870_880.0 / 10.24;
    for shape in [
        line([9.765625, 9.765625], [-52428796.875, 9.765625], "thick"),
        line([-m, -m / 3.0], [m, m / 3.0], "thick"),
    ] {
        let source = scene("square", vec![shape]);
        let expanded = expand_scene(&source).unwrap();
        for error in [
            check_bounds(source.canvas, &expanded).unwrap_err(),
            render::render_main(source.canvas, &expanded).unwrap_err(),
        ] {
            let RenderError::InvalidGeometry(e) = error else {
                panic!("expected shape error")
            };
            assert_eq!(e.kind, ErrorKind::Shape);
            assert_eq!(e.path, "shapes[0]");
            assert_eq!(e.detail, detail);
        }
    }
    for clipped in [false, true] {
        let mut group = json!({"type":"group","shapes":[{"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[{"type":"line","from":[10,10],"to":[20,10]}]}]});
        if clipped {
            group["allow_clip"] = json!(["left", "top", "right", "bottom"]);
        }
        let source = scene("square", vec![group]);
        let mut expanded = expand_scene(&source).unwrap();
        expanded[1].geometry.segments = vec![
            Segment::Move([100.0, 100.0]),
            Segment::Line([-536_870_880.0, 100.0]),
        ];
        let expected = format!("{detail}; repeat_index=1");
        for error in [
            check_bounds(source.canvas, &expanded).unwrap_err(),
            render::render_main(source.canvas, &expanded).unwrap_err(),
        ] {
            let RenderError::InvalidGeometry(e) = error else {
                panic!("expected shape error")
            };
            assert_eq!(e.kind, ErrorKind::Shape);
            assert_eq!(e.path, "shapes[0].shapes[0].shapes[0]");
            assert_eq!(e.detail, expected);
        }
    }
}
