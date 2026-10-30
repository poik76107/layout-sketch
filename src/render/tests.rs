use super::*;
use crate::geometry::expand_scene;
use crate::scene::parse_scene;
use image::{DynamicImage, ImageFormat, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;

fn source(canvas: &str, shapes: Vec<Value>) -> crate::scene::Scene {
    let input = json!({"schema_version":1,"canvas":canvas,"shapes":shapes}).to_string();
    parse_scene(input.as_bytes()).unwrap()
}
fn picture(shapes: Vec<Value>) -> RgbImage {
    let scene = source("square", shapes);
    let expanded = expand_scene(&scene).unwrap();
    render_main(scene.canvas, &expanded).unwrap()
}
fn line(from: [f64; 2], to: [f64; 2], width: &str) -> Value {
    json!({"type":"line","from":from,"to":to,"width":width})
}
fn pixel(image: &RgbImage, x: u32, y: u32) -> u8 {
    let pixel = image.get_pixel(x, y).0;
    assert_eq!(pixel[0], pixel[1]);
    assert_eq!(pixel[1], pixel[2]);
    pixel[0]
}
fn assert_white(image: &RgbImage) {
    assert!(image.pixels().all(|pixel| pixel.0 == [255, 255, 255]));
}
fn roundtrip(image: &RgbImage) {
    let mut buffer = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image.clone())
        .write_to(&mut buffer, ImageFormat::Png)
        .unwrap();
    let decoded = image::load_from_memory(buffer.get_ref()).unwrap();
    assert_eq!(decoded.color(), image::ColorType::Rgb8);
    assert_eq!(decoded.to_rgb8(), *image);
    assert!(decoded.to_rgba8().pixels().all(|p| p.0[3] == 255));
}
#[test]
fn render_canvas_rgb() {
    for (canvas, name, dimensions) in [
        (Canvas::Landscape, "landscape", (1536, 1024)),
        (Canvas::Portrait, "portrait", (1024, 1536)),
        (Canvas::Square, "square", (1024, 1024)),
    ] {
        let blank = render_main(canvas, &[]).unwrap();
        assert_eq!(blank.dimensions(), dimensions);
        assert_white(&blank);
        roundtrip(&blank);
        let off = source(name, vec![line([-100.0, -100.0], [-90.0, -100.0], "thick")]);
        let expanded = expand_scene(&off).unwrap();
        assert_white(&render_main(canvas, &expanded).unwrap());
        let visible = source(name, vec![line([25.0, 25.0], [35.0, 25.0], "thick")]);
        let img = render_main(canvas, &expand_scene(&visible).unwrap()).unwrap();
        assert_eq!(pixel(&img, 300, 256), 0);
        assert_eq!(pixel(&img, 10, 10), 255);
        for (x, y, value) in img.enumerate_pixels() {
            if !(250..=365).contains(&x) || !(250..=262).contains(&y) {
                assert_eq!(value.0, [255, 255, 255]);
            }
        }
        roundtrip(&img);
    }
    assert!(parse_scene(br#"{"schema_version":1,"shapes":[]}"#).is_err());
}
#[test]
fn render_paths() {
    let img = picture(vec![line([25.0, 25.0], [35.0, 25.0], "thin")]);
    assert_eq!(pixel(&img, 300, 256), 0);
    assert_eq!(pixel(&img, 30, 25), 255);
    let q = picture(vec![json!({"type":"path","d":"M20 40Q30 20 40 40"})]);
    assert_eq!(pixel(&q, 307, 307), 0);
    assert_eq!(pixel(&q, 307, 205), 255);
    assert_eq!(pixel(&q, 307, 410), 255);
    let c = picture(vec![json!({"type":"path","d":"M20 50C20 30 40 30 40 50"})]);
    assert_eq!(pixel(&c, 307, 358), 0);
    assert_eq!(pixel(&c, 205, 307), 255);
    assert_eq!(pixel(&c, 307, 512), 255);
    let open = picture(vec![
        json!({"type":"path","d":"M10 10L30 10L30 30 M50 10L70 10"}),
    ]);
    assert_eq!(pixel(&open, 205, 102), 0);
    assert_eq!(pixel(&open, 205, 205), 255);
    assert_eq!(pixel(&open, 410, 102), 255);
    // A wrongly connected Move would join (30,30) to (50,10); its midpoint is (40,20).
    assert_eq!(pixel(&open, 410, 205), 255);
    let arc_open = picture(vec![
        json!({"type":"arc","cx":20,"cy":20,"rx":10,"ry":10,"start":0,"sweep":90}),
    ]);
    assert_eq!(pixel(&arc_open, 256, 256), 255);
    let closed = picture(vec![json!({"type":"path","d":"M10 10L30 10L30 30Z"})]);
    assert_eq!(pixel(&closed, 205, 205), 0);
    let cases = [
        (
            json!({"type":"polyline","pts":[[10,10],[20,10],[20,20]]}),
            (154, 102),
        ),
        (
            json!({"type":"polygon","pts":[[10,10],[20,10],[20,20]]}),
            (154, 102),
        ),
        (
            json!({"type":"rect","x":10,"y":10,"w":10,"h":10}),
            (154, 102),
        ),
        (json!({"type":"circle","cx":20,"cy":20,"r":10}), (307, 205)),
        (
            json!({"type":"ellipse","cx":20,"cy":20,"rx":10,"ry":5,"angle":90}),
            (205, 307),
        ),
        (
            json!({"type":"arc","cx":20,"cy":20,"rx":10,"ry":10,"start":0,"sweep":90}),
            (307, 205),
        ),
        (
            json!({"type":"arc","cx":20,"cy":20,"rx":10,"ry":10,"start":0,"sweep":90,"chord":true}),
            (256, 256),
        ),
        (
            json!({"type":"curve","from":[20,50],"ctrl1":[20,30],"ctrl2":[40,30],"to":[40,50]}),
            (307, 358),
        ),
    ];
    for (node, (x, y)) in cases {
        let img = picture(vec![node]);
        assert!(pixel(&img, x, y) < 128, "{x},{y}");
    }
}
#[test]
fn render_layering() {
    let background = line([10.0, 20.0], [50.0, 20.0], "thick");
    for fill in ["white", "none", "black"] {
        let shape = json!({"type":"rect","x":20,"y":10,"w":20,"h":20,"fill":fill});
        let front = picture(vec![background.clone(), shape.clone()]);
        let behind = picture(vec![shape, background.clone()]);
        assert_eq!(
            pixel(&front, 307, 205),
            if fill == "white" { 255 } else { 0 }
        );
        assert_eq!(pixel(&behind, 307, 205), 0);
        assert_eq!(pixel(&front, 207, 150), 0);
        assert_eq!(
            pixel(&front, 307, 150),
            if fill == "black" { 0 } else { 255 }
        );
        assert_eq!(pixel(&front, 202, 150), 0);
        if fill == "white" {
            assert_ne!(front, behind);
        }
    }
    let group = json!({"type":"group","shapes":[{"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[{"type":"rect","x":20,"y":10,"w":20,"h":20,"fill":"white"}]}]});
    let img = picture(vec![
        background.clone(),
        group,
        line([25.0, 20.0], [35.0, 20.0], "thin"),
    ]);
    assert_eq!(pixel(&img, 230, 205), 255);
    assert_eq!(pixel(&img, 307, 205), 0);
    // Copy 0's upper border crosses copy 1's white interior at (30,25).
    let child = json!({"type":"rect","x":-10,"y":-5,"w":20,"h":15,"fill":"white"});
    let background = json!({"type":"rect","x":0,"y":0,"w":70,"h":70,"fill":"black"});
    let repeat = |count| json!({"type":"group","shapes":[{"type":"repeat_rotate","center":[30,30],"count":count,"step":90,"shapes":[child.clone()]}]});
    let after = line([50.0, 50.0], [60.0, 50.0], "thin");
    let first = picture(vec![background.clone(), repeat(1), after.clone()]);
    let both = picture(vec![background, repeat(2), after]);
    assert_eq!(pixel(&first, 307, 256), 0);
    assert_eq!(pixel(&both, 307, 256), 255);
    assert_eq!(pixel(&both, 563, 512), 0);
}
#[test]
fn render_even_odd() {
    let path = "M10 10L60 10L60 60L10 60Z M20 20L50 20L50 50L20 50Z";
    for reflected in [false, true] {
        for fill in ["black", "white"] {
            let p = json!({"type":"path","d":path,"fill":fill});
            let p = if reflected {
                json!({"type":"group","at":[70,0],"flip_x":true,"shapes":[p]})
            } else {
                p
            };
            let bg = json!({"type":"rect","x":0,"y":0,"w":80,"h":70,"fill":if fill=="white" {"black"} else {"white"}});
            let img = picture(vec![bg, p]);
            let x = if reflected { 563 } else { 154 };
            assert_eq!(pixel(&img, x, 154), if fill == "black" { 0 } else { 255 });
            assert_eq!(pixel(&img, 358, 358), if fill == "black" { 255 } else { 0 });
            let border = if reflected { 614 } else { 102 };
            assert_eq!(pixel(&img, border, 154), 0);
        }
    }
    let overlap = picture(vec![
        json!({"type":"path","d":"M10 10L40 10L40 40L10 40Z M30 10L60 10L60 40L30 40Z","fill":"black"}),
    ]);
    assert_eq!(pixel(&overlap, 205, 256), 0);
    assert_eq!(pixel(&overlap, 358, 256), 255);
    assert_eq!(pixel(&overlap, 512, 256), 0);
    let open = picture(vec![json!({"type":"path","d":"M10 10L40 10L40 40"})]);
    assert_eq!(pixel(&open, 307, 205), 255);
    let input=json!({"schema_version":1,"shapes":[{"type":"path","d":"M10 10L40 10L40 40","fill":"black"}]}).to_string();
    let err = parse_scene(input.as_bytes()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Shape);
    assert_eq!(err.path, "shapes[0].fill");
}
fn coverage(image: &RgbImage, vertical: bool, along: u32, center: u32) -> f64 {
    (center - 12..=center + 12)
        .map(|cross| {
            let value = if vertical {
                pixel(image, cross, along)
            } else {
                pixel(image, along, cross)
            };
            f64::from(255 - value) / 255.0
        })
        .sum()
}
#[test]
fn render_stroke_widths() {
    for (width, expected) in [("thick", 9.0), ("thin", 4.5)] {
        let fixtures = [
            line([15.0, 25.0], [35.0, 25.0], width),
            json!({"type":"group","scale":1,"shapes":[line([15.0,25.0],[35.0,25.0],width)]}),
            json!({"type":"group","scale":2,"shapes":[line([7.5,12.5],[17.5,12.5],width)]}),
            json!({"type":"group","flip_x":true,"rotate":180,"at":[0,50],"shapes":[line([15.0,25.0],[35.0,25.0],width)]}),
            json!({"type":"repeat_rotate","center":[0,25],"count":1,"shapes":[line([15.0,0.0],[35.0,0.0],width)]}),
        ];
        for node in fixtures {
            let img = picture(vec![node]);
            let measured = coverage(&img, false, 256, 256);
            assert!(
                (measured - expected).abs() < 0.15,
                "{measured} vs {expected}"
            );
            assert!(img.pixels().any(|p| p.0[0] > 0 && p.0[0] < 255));
            assert!(img.pixels().all(|p| p.0[0] == p.0[1] && p.0[1] == p.0[2]));
        }
        for child in [
            line([15.0, 25.0], [35.0, 25.0], width),
            json!({"type":"repeat_rotate","center":[0,25],"count":1,"shapes":[line([15.0,0.0],[35.0,0.0],width)]}),
        ] {
            let img = picture(vec![json!({"type":"group","scale":2,"shapes":[child]})]);
            assert!((coverage(&img, false, 500, 512) - expected).abs() < 0.15);
            assert_eq!(pixel(&img, 500, 512), 0);
            assert_eq!(pixel(&img, 250, 512), 255);
        }
        let img = picture(vec![line([25.0, 15.0], [25.0, 35.0], width)]);
        assert!((coverage(&img, true, 256, 256) - expected).abs() < 0.15);
    }
}
#[test]
fn render_round_strokes() {
    for width in ["thick", "thin"] {
        let img = picture(vec![line([25.0, 25.0], [40.0, 25.0], width)]);
        assert_eq!(pixel(&img, 254, 256), 0);
        assert_eq!(pixel(&img, 252, 251), 255);
        if width == "thin" {
            assert_eq!(pixel(&img, 253, 254), 255);
        }
        let bend = picture(vec![
            json!({"type":"polyline","pts":[[10,25],[25,25],[25,40]],"width":width}),
        ]);
        let corner = if width == "thick" {
            (258, 253)
        } else {
            (257, 254)
        };
        // The probe is inside the endpoint disk and outside the bevel triangle;
        // AA is allowed up to 180, and the distant probe excludes a miter spike.
        assert!(pixel(&bend, corner.0, corner.1) < 180, "round join {width}");
        assert_eq!(pixel(&bend, 261, 250), 255);
        if width == "thick" {
            // This pixel lies outside the round disk but within the miter square.
            assert!(pixel(&bend, 259, 252) > 200);
        }
        let acute = picture(vec![
            json!({"type":"polyline","pts":[[10,25],[25,25],[15,30]],"width":width}),
        ]);
        assert_eq!(pixel(&acute, 265, 253), 255);
        let separate = picture(vec![
            json!({"type":"path","d":"M25 25L40 25 M25 40L40 40","width":width}),
        ]);
        assert_eq!(pixel(&separate, 254, 256), 0);
        assert_eq!(pixel(&separate, 254, 409), 0);
        let curve = picture(vec![
            json!({"type":"curve","from":[25,25],"ctrl1":[30,25],"ctrl2":[35,25],"to":[40,25],"width":width}),
        ]);
        assert_eq!(pixel(&curve, 254, 256), 0);
    }
    let closed = picture(vec![
        json!({"type":"polygon","pts":[[25,25],[40,25],[40,40]],"fill":"white"}),
    ]);
    assert!(pixel(&closed, 254, 254) < 255);
    // Closing vertex at (256,256): the first probe is inside the round join
    // but outside a bevel; the second is outside the disk but inside a miter.
    let closing = picture(vec![
        json!({"type":"rect","x":25,"y":25,"w":15,"h":15,"fill":"white"}),
    ]);
    assert!(pixel(&closing, 253, 253) < 180);
    assert!(pixel(&closing, 252, 252) > 200);
}

fn assert_invalid(item: &ExpandedPrimitive, path: &str, detail: &str) {
    let result = render_main(Canvas::Square, std::slice::from_ref(item));
    let Err(RenderError::InvalidGeometry(error)) = result else {
        panic!("expected InvalidGeometry")
    };
    assert_eq!(error.kind, ErrorKind::Shape);
    assert_eq!(error.path, path);
    assert_eq!(error.detail, detail);
}
#[test]
fn render_large_finite_rectangle() {
    let rect = |n: f64| json!({"type":"rect","x":-n,"y":-n,"w":2.0*n,"h":2.0*n,"fill":"black"});
    let scene = source("square", vec![rect(1e7)]);
    let expanded = expand_scene(&scene).unwrap();
    assert_invalid(&expanded[0], "shapes[0]", EXTENT_DETAIL);
    let image = picture(vec![rect(1e5)]);
    assert!(image.pixels().all(|p| p.0 == [0, 0, 0]));
    let scene = source("square", vec![rect(6e7)]);
    let expanded = expand_scene(&scene).unwrap();
    assert_invalid(
        &expanded[0],
        "shapes[0]",
        "coordinate exceeds raster backend numeric range",
    );
}

#[test]
fn render_large_finite_diagonal() {
    let image = picture(vec![line([0.0, 0.0], [90.0, 30.0], "thick")]);
    assert_eq!(pixel(&image, 300, 100), 0);
    assert_eq!(pixel(&image, 300, 300), 255);
    let bad = line([0.0, 0.0], [1e9, 1e9 / 3.0], "thick");
    let scene = source("square", vec![bad.clone()]);
    let expanded = expand_scene(&scene).unwrap();
    assert_invalid(
        &expanded[0],
        "shapes[0]",
        "coordinate exceeds raster backend numeric range",
    );
    let scene = source("square", vec![line([0.0, 0.0], [90.0, 30.0], "thick"), bad]);
    let expanded = expand_scene(&scene).unwrap();
    let Err(RenderError::InvalidGeometry(e)) = render_main(scene.canvas, &expanded) else {
        panic!("expected shape error")
    };
    assert_eq!(e.kind, ErrorKind::Shape);
    assert_eq!(e.path, "shapes[1]");
    assert_eq!(e.detail, "coordinate exceeds raster backend numeric range");
}

#[test]
fn render_backend_scan_range() {
    let scene = source("square", vec![line([0.0, 0.0], [10.0, 10.0], "thick")]);
    let mut item = expand_scene(&scene).unwrap().remove(0);
    let limit = 536_870_880.0;
    item.geometry.fill = Fill::Black;
    item.geometry.segments = vec![
        Segment::Move([-limit, -limit]),
        Segment::Line([limit, -limit]),
        Segment::Line([limit, limit]),
        Segment::Line([-limit, limit]),
        Segment::Close,
    ];
    assert_invalid(&item, "shapes[0]", EXTENT_DETAIL);
    item.geometry.fill = Fill::None;
    for end in [[200.0, 100.0], [100.0, 200.0]] {
        let start = [100.0, 100.0];
        item.geometry.segments = vec![Segment::Move(start), Segment::Line(end)];
        render_main(Canvas::Square, std::slice::from_ref(&item)).unwrap();
    }
    for sign in [-1.0, 1.0] {
        for axis in 0..2 {
            let mut outside = [100.0, 100.0];
            outside[axis] = sign * 536_870_912.0;
            for position in 0..7 {
                let a = [100.0, 100.0];
                let b = [200.0, 200.0];
                item.geometry.segments = match position {
                    0 => vec![Segment::Move(outside), Segment::Line(b)],
                    1 => vec![Segment::Move(a), Segment::Line(outside)],
                    2 => vec![Segment::Move(a), Segment::Quadratic(outside, b)],
                    3 => vec![Segment::Move(a), Segment::Cubic(outside, a, b)],
                    4 => vec![Segment::Move(a), Segment::Cubic(a, outside, b)],
                    5 => vec![Segment::Move(a), Segment::Quadratic(a, outside)],
                    _ => vec![Segment::Move(a), Segment::Cubic(a, b, outside)],
                };
                assert_invalid(
                    &item,
                    "shapes[0]",
                    "coordinate exceeds raster backend numeric range",
                );
            }
        }
    }
    let rect = json!({"type":"rect","x":-10,"y":-10,"w":120,"h":120,"fill":"black"});
    let image = picture(vec![rect.clone()]);
    assert!(image.pixels().all(|p| p.0 == [0, 0, 0]));
    let mut allowed = rect;
    allowed["allow_clip"] = json!(["left", "top", "right", "bottom"]);
    assert_eq!(image, picture(vec![allowed]));
}
#[test]
fn render_numeric_failures() {
    let huge = source("square", vec![line([0.0, 0.0], [1e40, 0.0], "thick")]);
    let expanded = expand_scene(&huge).unwrap();
    assert_invalid(
        &expanded[0],
        "shapes[0]",
        "coordinate cannot be represented as finite f32",
    );
    let envelope = source("square", vec![line([0.0, 0.0], [1e37, 0.0], "thick")]);
    let expanded = expand_scene(&envelope).unwrap();
    assert_invalid(
        &expanded[0],
        "shapes[0]",
        "coordinate exceeds raster backend numeric range",
    );
    let repeated = source(
        "square",
        vec![
            json!({"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[{"type":"path","d":"M0 0Q1 2 2 0C2 1 3 1 3 0"}]}),
        ],
    );
    let mut expanded = expand_scene(&repeated).unwrap();
    expanded[1].geometry.segments[1] = Segment::Quadratic([f64::NAN, 0.0], [20.48, 0.0]);
    assert_invalid(
        &expanded[1],
        "shapes[0].shapes[0]",
        "coordinate cannot be represented as finite f32; repeat_index=1",
    );
    assert!(matches!(
        render_main(Canvas::Square, &expanded),
        Err(RenderError::InvalidGeometry(_))
    ));
    expanded[1].geometry.segments[1] = Segment::Quadratic([10.24, 20.48], [20.48, 0.0]);
    expanded[1].geometry.segments[2] = Segment::Cubic([0.0, 0.0], [0.0, f64::NAN], [30.72, 0.0]);
    assert_invalid(
        &expanded[1],
        "shapes[0].shapes[0]",
        "coordinate cannot be represented as finite f32; repeat_index=1",
    );
    let invalid_path = source(
        "square",
        vec![
            json!({"type":"repeat_rotate","center":[0,0],"count":2,"shapes":[line([0.0,0.0],[1.0,0.0],"thick")]}),
        ],
    );
    let mut empty = expand_scene(&invalid_path).unwrap();
    empty[1].geometry.segments = vec![Segment::Move([0.0, 0.0])];
    let Err(RenderError::Raster {
        source_path,
        repeat_index,
        detail,
    }) = render_main(Canvas::Square, &empty)
    else {
        panic!("expected raster failure")
    };
    assert_eq!(source_path.as_deref(), Some("shapes[0].shapes[0]"));
    assert_eq!(repeat_index, Some(1));
    assert_eq!(detail, "cannot construct raster path");
    let crossing = picture(vec![
        line([-10.0, 20.0], [10.0, 20.0], "thick"),
        line([20.0, -10.0], [20.0, 10.0], "thick"),
    ]);
    assert_eq!(pixel(&crossing, 0, 205), 0);
    assert_eq!(pixel(&crossing, 205, 0), 0);
    let off = picture(vec![line([-10.0, -10.0], [-5.0, -10.0], "thin")]);
    assert_white(&off);
}
#[test]
fn render_determinism_examples() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/layout-sketch/assets/examples"
    );
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|v| v.unwrap().path())
        .filter(|p| p.extension().is_some_and(|v| v == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 6);
    for path in files {
        let input = std::fs::read(path).unwrap();
        let scene = parse_scene(&input).unwrap();
        let expanded = expand_scene(&scene).unwrap();
        let image = render_main(scene.canvas, &expanded).unwrap();
        assert_eq!(image, render_main(scene.canvas, &expanded).unwrap());
        assert_eq!(scene, parse_scene(&input).unwrap());
        assert_eq!(expanded, expand_scene(&scene).unwrap());
        assert_eq!(image.dimensions(), canvas_dimensions(scene.canvas));
        assert_eq!(pixel(&image, 0, 0), 255);
        assert!(image.pixels().any(|p| p.0[0] < 128));
        assert!(image.pixels().all(|p| p.0[0] == p.0[1] && p.0[1] == p.0[2]));
        roundtrip(&image);
    }
    let a = picture(vec![
        json!({"id":"one","type":"line","from":[10,10],"to":[20,10],"allow_clip":["top"]}),
    ]);
    let b = picture(vec![
        json!({"id":"two","type":"line","from":[10,10],"to":[20,10],"allow_clip":["left"]}),
    ]);
    assert_eq!(a, b);
    assert_eq!(pixel(&a, 154, 102), 0);
    for (x, y, p) in a.enumerate_pixels() {
        if !(95..=210).contains(&x) || !(95..=110).contains(&y) {
            assert_eq!(p.0, [255, 255, 255]);
        }
    }
    let background = line([10.0, 20.0], [50.0, 20.0], "thick");
    let shape = json!({"type":"rect","x":20,"y":10,"w":20,"h":20,"fill":"white"});
    assert_ne!(
        picture(vec![background.clone(), shape.clone()]),
        picture(vec![shape, background])
    );
}

#[test]
fn render_bounds_preparation_pin() {
    let image = picture(vec![
        line([10.0, 25.0], [40.0, 25.0], "thick"),
        json!({"type":"rect","x":20,"y":20,"w":10,"h":10,"fill":"white"}),
    ]);
    assert_eq!(pixel(&image, 150, 256), 0);
    assert_eq!(pixel(&image, 256, 256), 255);
    assert_eq!(pixel(&image, 205, 230), 0);
    assert_eq!(pixel(&image, 20, 20), 255);
}

#[test]
fn render_preview_resize() {
    for (canvas, dims) in [
        (Canvas::Landscape, (384, 256)),
        (Canvas::Portrait, (256, 384)),
        (Canvas::Square, (256, 256)),
    ] {
        let mut main = render_main(canvas, &[]).unwrap();
        main.put_pixel(100, 100, image::Rgb([0, 0, 0]));
        let before = main.clone();
        assert_eq!(
            render_preview(&main, 0.25, false).unwrap().dimensions(),
            dims
        );
        assert_eq!(render_preview(&main, 1.0, false).unwrap(), main);
        assert_eq!(
            render_preview(&main, 0.5, false).unwrap().dimensions(),
            (dims.0 * 2, dims.1 * 2)
        );
        assert_eq!(main, before);
    }
    let main = render_main(Canvas::Landscape, &[]).unwrap();
    assert_eq!(
        render_preview(&main, 0.333, false).unwrap().dimensions(),
        (511, 341)
    );
    assert_eq!(
        render_preview(&main, 0.1, false).unwrap().dimensions(),
        (154, 102)
    );
}

#[test]
fn render_preview_grid() {
    let main = image::RgbImage::from_pixel(1024, 1024, image::Rgb([255, 255, 255]));
    for scale in [1.0, 0.25, 0.333] {
        let grid = render_preview(&main, scale, true).unwrap();
        let position = 102.4 * f64::from(grid.width()) / 1024.0;
        let base = position.floor() as i32;
        let y = 10;
        let mut coverage = 0.0;
        let mut horizontal_coverage = 0.0;
        for x in base - 2..=base + 2 {
            coverage += (255.0 - f64::from(grid.get_pixel(x as u32, y).0[0])) / 55.0;
            horizontal_coverage += (255.0 - f64::from(grid.get_pixel(y, x as u32).0[0])) / 55.0;
        }
        assert!(
            (coverage - 1.0).abs() < 0.1,
            "scale={scale} coverage={coverage}"
        );
        assert!((horizontal_coverage - 1.0).abs() < 0.1);
        assert_eq!(grid.get_pixel(10, 10).0, [255, 255, 255]);
    }
    let grid = render_preview(&main, 0.333, true).unwrap();
    assert_eq!(grid.get_pixel(170, 10).0, [200, 200, 200]);
    assert!(grid.pixels().any(|p| p.0[0] > 200 && p.0[0] < 255));
    assert!(main.pixels().all(|p| p.0 == [255, 255, 255]));
}

const EXTENT_DETAIL: &str = "path bounds exceed raster precision limit of 16777216 px; reduce the shape size or coordinate magnitudes";

#[test]
fn render_backend_extent_limit() {
    let scene = source("square", vec![line([0.0, 0.0], [1.0, 0.0], "thick")]);
    let item = &expand_scene(&scene).unwrap()[0];
    for transpose in [false, true] {
        for (end, expected) in [(104.0, 16_777_216.0), (105.0, 16_777_217.0)] {
            let mut b = tiny_skia::PathBuilder::new();
            let (x, y, w, h) = if transpose {
                (20.0, -16_777_112.0, 10.0, end + 16_777_112.0)
            } else {
                (-16_777_112.0, 20.0, end + 16_777_112.0, 10.0)
            };
            // Build from edges: constructing a width in f32 would round the +1 case.
            if transpose {
                b.move_to(x, y);
                b.line_to(x + w, y);
                b.line_to(x + w, end);
                b.line_to(x, end);
            } else {
                b.move_to(x, y);
                b.line_to(end, y);
                b.line_to(end, y + h);
                b.line_to(x, y + h);
            }
            b.close();
            let path = b.finish().unwrap();
            let bounds = path.bounds();
            let extent = if transpose {
                f64::from(bounds.bottom()).ceil() - f64::from(bounds.top()).floor()
            } else {
                f64::from(bounds.right()).ceil() - f64::from(bounds.left()).floor()
            };
            assert_eq!(extent, expected);
            assert_eq!(
                check_path_extent(&path, item).is_ok(),
                expected == 16_777_216.0
            );
        }
        for (start, end, expected) in [
            (-16_777_107.0, 99.5, 16_777_216.0),
            (-16_777_107.0, 100.5, 16_777_217.0),
            (-8_388_603.0, 8_388_603.0, 16_777_216.0),
        ] {
            let mut b = tiny_skia::PathBuilder::new();
            if transpose {
                b.move_to(100.0, start);
                b.line_to(100.0, end);
            } else {
                b.move_to(start, 100.0);
                b.line_to(end, 100.0);
            }
            let stroke = tiny_skia::Stroke {
                width: 9.0,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Default::default()
            };
            let outline = b.finish().unwrap().stroke(&stroke, 1.0).unwrap();
            let bounds = outline.bounds();
            let (lo, hi) = if transpose {
                (bounds.top(), bounds.bottom())
            } else {
                (bounds.left(), bounds.right())
            };
            assert_eq!(f64::from(hi).ceil() - f64::from(lo).floor(), expected);
            if start == -8_388_603.0 {
                assert_eq!((lo, hi), (-8_388_607.5, 8_388_607.5));
            }
            assert_eq!(
                check_path_extent(&outline, item).is_ok(),
                expected == 16_777_216.0
            );
        }
    }
    // Control bounds exceed the cap while the actual curve and its outline do not.
    let mut item = expand_scene(&scene).unwrap().remove(0);
    item.geometry.segments = vec![
        Segment::Move([100.0, 100.0]),
        Segment::Quadratic([16_777_320.0, 100.0], [100.0, 200.0]),
    ];
    for fill in [Fill::Black, Fill::White] {
        item.geometry.fill = fill;
        assert_invalid(&item, "shapes[0]", EXTENT_DETAIL);
    }
    item.geometry.fill = Fill::None;
    render_main(Canvas::Square, &[item]).unwrap();
}

#[test]
fn render_backend_extent_regressions() {
    let m = 536_870_880.0 / 10.24;
    for shape in [
        line([9.765625, 9.765625], [-52428796.875, 9.765625], "thick"),
        line([-m, -m / 3.0], [m, m / 3.0], "thick"),
        line([m, m / 3.0], [-m, -m / 3.0], "thick"),
        json!({"type":"rect","x":-1e7,"y":-1e7,"w":2e7,"h":2e7,"fill":"black"}),
    ] {
        let scene = source("square", vec![shape]);
        let expanded = expand_scene(&scene).unwrap();
        assert_invalid(&expanded[0], "shapes[0]", EXTENT_DETAIL);
    }
    let image = picture(vec![line([-10.0, 20.0], [10.0, 20.0], "thick")]);
    assert_eq!(pixel(&image, 0, 205), 0);
}
