use image::imageops::FilterType;
use layout_sketch::{bounds, geometry, render, scene};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const SIMPLE: &str = r#"{"schema_version":1,"canvas":"square","shapes":[{"type":"line","from":[10,20],"to":[60,20]},{"type":"rect","x":20,"y":10,"w":10,"h":20,"fill":"white"}]}"#;
fn run(dir: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sketch"))
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > until {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("scene.json"), SIMPLE).unwrap();
    dir
}
fn ok(result: &Output) {
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
}
fn decoded(path: &Path) -> image::RgbImage {
    let image = image::load_from_memory(&fs::read(path).unwrap()).unwrap();
    assert_eq!(image.color(), image::ColorType::Rgb8);
    image.to_rgb8()
}
#[cfg(unix)]
#[test]
fn cli_output_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = setup();
    for (mask, expected, overwrite) in [
        ("022", 0o644, false),
        ("077", 0o600, true),
        ("022", 0o644, true),
    ] {
        let mut command = Command::new("sh");
        command.current_dir(directory.path()).args([
            "-c",
            "umask \"$1\"; shift; exec \"$@\"",
            "sketch-test",
            mask,
            env!("CARGO_BIN_EXE_sketch"),
            "scene.json",
            "-o",
            "main",
            "--preview",
            "preview",
        ]);
        if overwrite {
            command.arg("--overwrite");
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(30);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() > until {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        ok(&child.wait_with_output().unwrap());
        for name in ["main", "preview"] {
            let path = directory.path().join(name);
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                expected,
                "{name}, umask {mask}"
            );
            decoded(&path);
        }
    }
}
#[test]
fn cli_arguments() {
    let dir = setup();
    fs::write(dir.path().join("a"), b"main sentinel").unwrap();
    fs::write(dir.path().join("b"), b"preview sentinel").unwrap();
    for args in [
        vec![],
        vec!["scene.json"],
        vec!["-o", "x"],
        vec!["scene.json", "-o"],
        vec!["scene.json", "-o", "a", "--output", "b"],
        vec!["scene.json", "-o", "a", "--grid"],
        vec!["scene.json", "-o", "a", "--preview-scale", "0.25"],
        vec!["scene.json", "-o", "a", "--canvas", "square"],
        vec!["scene.json", "other", "-o", "a"],
        vec!["--help", "--grid"],
        vec!["scene.json", "-o", "a", "--overwrite", "--overwrite"],
        vec!["scene.json", "-o", "a", "--preview"],
        vec!["scene.json", "-o", "a", "--preview-scale"],
        vec!["scene.json", "-o", "a", "--preview", "b", "--preview", "c"],
        vec![
            "scene.json",
            "-o",
            "a",
            "--preview",
            "b",
            "--grid",
            "--grid",
        ],
        vec![
            "scene.json",
            "-o",
            "a",
            "--preview",
            "b",
            "--preview-scale",
            "1",
            "--preview-scale",
            "0.5",
        ],
    ] {
        let mut args = args;
        if !args.contains(&"--overwrite") {
            args.push("--overwrite");
        }
        let result = run(dir.path(), &args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("ERROR argument:"));
        assert_eq!(fs::read(dir.path().join("a")).unwrap(), b"main sentinel");
        assert_eq!(fs::read(dir.path().join("b")).unwrap(), b"preview sentinel");
    }
    for scale in ["nan", "inf", "0.099", "1.001", "garbage"] {
        let result = run(
            dir.path(),
            &[
                "scene.json",
                "-o",
                "a",
                "--preview",
                "b",
                "--preview-scale",
                scale,
                "--overwrite",
            ],
        );
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(fs::read(dir.path().join("a")).unwrap(), b"main sentinel");
        assert_eq!(fs::read(dir.path().join("b")).unwrap(), b"preview sentinel");
    }
    for scale in ["0.1", "0.25", "0.5", "1"] {
        let result = run(
            dir.path(),
            &[
                "--output",
                "a",
                "--preview-scale",
                scale,
                "--preview",
                "b",
                "--overwrite",
                "scene.json",
            ],
        );
        ok(&result);
    }
}
#[test]
fn cli_main() {
    let dir = setup();
    for (canvas, size) in [
        ("square", (1024, 1024)),
        ("landscape", (1536, 1024)),
        ("portrait", (1024, 1536)),
    ] {
        let input = SIMPLE.replace("square", canvas);
        fs::write(dir.path().join("scene.json"), &input).unwrap();
        let result = run(
            dir.path(),
            &["scene.json", "-o", "out file.dat", "--overwrite"],
        );
        ok(&result);
        let image = decoded(&dir.path().join("out file.dat"));
        assert_eq!(image.dimensions(), size);
        let scene = scene::parse_scene(input.as_bytes()).unwrap();
        let expanded = geometry::expand_scene(&scene).unwrap();
        assert_eq!(image, render::render_main(scene.canvas, &expanded).unwrap());
        assert_eq!(image.get_pixel(150, 205).0, [0, 0, 0]);
        assert_eq!(image.get_pixel(256, 205).0, [255, 255, 255]);
        ok(&run(
            dir.path(),
            &["scene.json", "-o", "out file.dat", "--overwrite"],
        ));
        assert_eq!(decoded(&dir.path().join("out file.dat")), image);
        assert_eq!(
            fs::read(dir.path().join("scene.json")).unwrap(),
            input.as_bytes()
        );
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn cli_preview_resize() {
    let dir = setup();
    for canvas in ["square", "landscape", "portrait"] {
        fs::write(
            dir.path().join("scene.json"),
            SIMPLE.replace("square", canvas),
        )
        .unwrap();
        for scale in ["0.1", "0.25", "0.333", "0.5", "1"] {
            ok(&run(
                dir.path(),
                &[
                    "scene.json",
                    "-o",
                    "main",
                    "--preview",
                    "prev",
                    "--preview-scale",
                    scale,
                    "--overwrite",
                ],
            ));
            let main = decoded(&dir.path().join("main"));
            let preview = decoded(&dir.path().join("prev"));
            let factor: f64 = scale.parse().unwrap();
            let w = (main.width() as f64 * factor + 0.5).floor() as u32;
            let h = (main.height() as f64 * factor + 0.5).floor() as u32;
            assert_eq!(
                preview,
                image::imageops::resize(&main, w, h, FilterType::Lanczos3)
            );
            if scale == "0.333" && canvas == "landscape" {
                assert_eq!((w, h), (511, 341));
            }
            if scale == "0.25" {
                assert_ne!(
                    preview,
                    image::imageops::resize(&main, w, h, FilterType::Nearest)
                );
                assert_ne!(
                    preview,
                    image::imageops::resize(&main, w, h, FilterType::Triangle)
                );
            }
        }
    }
}
#[test]
fn cli_preview_grid() {
    let dir = setup();
    ok(&run(
        dir.path(),
        &["scene.json", "-o", "main", "--preview", "prev"],
    ));
    let main = decoded(&dir.path().join("main"));
    let prev = decoded(&dir.path().join("prev"));
    ok(&run(
        dir.path(),
        &[
            "scene.json",
            "-o",
            "main",
            "--preview",
            "prev",
            "--grid",
            "--overwrite",
        ],
    ));
    assert_eq!(decoded(&dir.path().join("main")), main);
    let grid = decoded(&dir.path().join("prev"));
    assert_ne!(grid, prev);
    assert_eq!(grid.get_pixel(10, 10).0, [255, 255, 255]);
    assert!(grid.get_pixel(25, 10).0[0] < 255);
    assert_eq!(
        prev,
        image::imageops::resize(&main, 256, 256, FilterType::Lanczos3)
    );
    ok(&run(
        dir.path(),
        &[
            "scene.json",
            "-o",
            "main",
            "--preview",
            "prev",
            "--grid",
            "--preview-scale",
            "0.333",
            "--overwrite",
        ],
    ));
    assert_eq!(decoded(&dir.path().join("main")), main);
    let fractional = decoded(&dir.path().join("prev"));
    assert_eq!(fractional.dimensions(), (341, 341));
    assert_eq!(fractional.get_pixel(170, 10).0, [200, 200, 200]);
}
#[cfg(unix)]
#[test]
fn cli_path_identity() {
    use std::os::unix::fs::symlink;
    let dir = setup();
    let d = dir.path();
    symlink(d, d.join("alias")).unwrap();
    fs::hard_link(d.join("scene.json"), d.join("hard")).unwrap();
    symlink("scene.json", d.join("sym")).unwrap();
    for overwrite in [false, true] {
        for args in [
            vec!["scene.json", "-o", "./scene.json"],
            vec!["scene.json", "-o", "hard"],
            vec!["scene.json", "-o", "sym"],
            vec!["scene.json", "-o", "main", "--preview", "sym"],
            vec!["scene.json", "-o", "main", "--preview", "hard"],
            vec!["scene.json", "-o", "alias/out", "--preview", "out"],
        ] {
            let mut args = args;
            if overwrite {
                args.push("--overwrite");
            }
            let result = run(d, &args);
            assert_eq!(
                result.status.code(),
                Some(2),
                "{args:?} {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read(d.join("scene.json")).unwrap(), SIMPLE.as_bytes());
        }
    }
    fs::write(d.join("referent"), b"keep").unwrap();
    symlink("referent", d.join("target")).unwrap();
    assert_eq!(
        run(d, &["scene.json", "-o", "target"]).status.code(),
        Some(3)
    );
    assert_eq!(fs::read(d.join("referent")).unwrap(), b"keep");
    ok(&run(d, &["scene.json", "-o", "target", "--overwrite"]));
    assert_eq!(fs::read(d.join("referent")).unwrap(), b"keep");
    assert!(
        !fs::symlink_metadata(d.join("target"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    symlink("missing", d.join("dangling")).unwrap();
    assert_eq!(
        run(d, &["scene.json", "-o", "dangling"]).status.code(),
        Some(3)
    );
    ok(&run(d, &["scene.json", "-o", "dangling", "--overwrite"]));
    fs::write(d.join("first"), b"preserved").unwrap();
    fs::hard_link(d.join("first"), d.join("second")).unwrap();
    for overwrite in [false, true] {
        let mut args = vec!["scene.json", "-o", "first", "--preview", "second"];
        if overwrite {
            args.push("--overwrite");
        }
        assert_eq!(run(d, &args).status.code(), Some(2));
        assert_eq!(fs::read(d.join("first")).unwrap(), b"preserved");
        assert_eq!(fs::read(d.join("second")).unwrap(), b"preserved");
    }
}
#[test]
fn cli_overwrite() {
    for which in ["main", "prev", "both"] {
        let dir = setup();
        let d = dir.path();
        if which != "prev" {
            fs::write(d.join("main"), b"old main").unwrap();
        }
        if which != "main" {
            fs::write(d.join("prev"), b"old prev").unwrap();
        }
        assert_eq!(
            run(d, &["scene.json", "-o", "main", "--preview", "prev"])
                .status
                .code(),
            Some(3)
        );
        if which != "prev" {
            assert_eq!(fs::read(d.join("main")).unwrap(), b"old main");
        } else {
            assert!(!d.join("main").exists());
        }
        if which != "main" {
            assert_eq!(fs::read(d.join("prev")).unwrap(), b"old prev");
        } else {
            assert!(!d.join("prev").exists());
        }
        ok(&run(
            d,
            &[
                "scene.json",
                "-o",
                "main",
                "--preview",
                "prev",
                "--overwrite",
            ],
        ));
        decoded(&d.join("main"));
        decoded(&d.join("prev"));
    }
}
#[test]
fn cli_input_errors() {
    let dir = setup();
    let d = dir.path();
    fs::write(d.join("out"), b"sentinel").unwrap();
    for (input, part) in [
        ("{", "ERROR"),
        (
            r#"{"schema_version":1,"shapes":[{"type":"line","from":[0,0],"to":[1,1],"unknown":1}]}"#,
            "unknown",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"line","from":[0,0],"angle":0,"length":0}]}"#,
            "shapes[0].length",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"path","d":"M0 0 H1"}]}"#,
            "offset=5",
        ),
        (
            r#"{"schema_version":1,"shapes":[{"type":"line","from":[0,0],"to":[1e40,1]}]}"#,
            "shapes[0]",
        ),
    ] {
        fs::write(d.join("scene.json"), input).unwrap();
        let r = run(d, &["scene.json", "-o", "out", "--overwrite"]);
        assert_eq!(
            r.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&r.stderr)
        );
        assert!(
            String::from_utf8_lossy(&r.stderr).contains(part),
            "expected {part}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        assert!(!String::from_utf8_lossy(&r.stdout).contains("OK "));
        assert_eq!(fs::read(d.join("out")).unwrap(), b"sentinel");
    }
    let children = vec![serde_json::json!({"type":"line","from":[0,0],"to":[1,1]}); 40];
    let excessive = serde_json::json!({"schema_version":1,"shapes":[{"type":"repeat_rotate","count":256,"center":[0,0],"shapes":children}]}).to_string();
    fs::write(d.join("scene.json"), excessive).unwrap();
    let result = run(d, &["scene.json", "-o", "out", "--overwrite"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("10000"));
    assert_eq!(fs::read(d.join("out")).unwrap(), b"sentinel");
    let mut input = SIMPLE.as_bytes().to_vec();
    input.resize(scene::MAX_INPUT_BYTES, b' ');
    fs::write(d.join("scene.json"), &input).unwrap();
    ok(&run(d, &["scene.json", "-o", "out", "--overwrite"]));
    input.push(b' ');
    fs::write(d.join("scene.json"), input).unwrap();
    let r = run(d, &["scene.json", "-o", "other"]);
    assert_eq!(r.status.code(), Some(2));
    assert!(!d.join("other").exists());
}
#[test]
fn cli_io_failures() {
    let dir = setup();
    let d = dir.path();
    fs::create_dir(d.join("directory")).unwrap();
    for args in [
        vec!["missing", "-o", "out"],
        vec!["scene.json", "-o", "missing/out"],
        vec!["scene.json", "-o", "directory"],
    ] {
        let r = run(d, &args);
        assert_eq!(
            r.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&r.stderr)
        );
        assert!(!String::from_utf8_lossy(&r.stdout).contains("OK "));
    }
    #[cfg(target_os = "linux")]
    {
        fs::write(
            d.join("scene.json"),
            r#"{"schema_version":1,"shapes":[{"type":"line","from":[0,20],"to":[0,80]}]}"#,
        )
        .unwrap();
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_sketch"))
            .current_dir(d)
            .args(["scene.json", "-o", "out"])
            .stdout(Stdio::from(full))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(!d.join("out").exists());
    }
}
#[test]
fn cli_diagnostics() {
    let dir = setup();
    let d = dir.path();
    let input = r#"{"schema_version":1,"canvas":"square","shapes":[{"type":"line","id":"axis","from":[0,20],"to":[0,80]},{"type":"group","shapes":[{"type":"repeat_rotate","center":[0,0],"count":2,"step":0,"shapes":[{"type":"line","from":[0,0],"to":[100,100],"allow_clip":["top"]}]}]}]}"#;
    fs::write(d.join("scene.json"), input).unwrap();
    let s = scene::parse_scene(input.as_bytes()).unwrap();
    let p = geometry::expand_scene(&s).unwrap();
    let warnings = bounds::check_bounds(s.canvas, &p).unwrap();
    assert_eq!(warnings.len(), 7);
    assert_eq!(
        warnings[0].to_string(),
        "WARN OUTSIDE path=shapes[0] id=axis edge=left excess_px=4.5"
    );
    assert_eq!(warnings[1].repeat_index, Some(0));
    assert_eq!(warnings[4].repeat_index, Some(1));
    let r = run(
        d,
        &[
            "scene.json",
            "-o",
            "main file",
            "--preview",
            "pre\"view",
            "--grid",
        ],
    );
    ok(&r);
    let text = String::from_utf8(r.stdout).unwrap();
    let lines: Vec<_> = text.lines().collect();
    for (line, warning) in lines.iter().zip(&warnings) {
        assert_eq!(*line, warning.to_string());
    }
    assert_eq!(
        lines[7],
        "OK output=\"main file\" size=1024x1024 warnings=7"
    );
    assert_eq!(
        lines[8],
        "OK preview=\"pre\\\"view\" size=256x256 scale=0.25 grid=true"
    );
    let failure = run(d, &["scene.json", "-o", "main file"]);
    assert_eq!(failure.status.code(), Some(3));
    assert!(!String::from_utf8_lossy(&failure.stdout).contains("OK "));
    let no_clip = input.replace(",\"allow_clip\":[\"top\"]", "");
    fs::write(d.join("scene.json"), &no_clip).unwrap();
    let unmasked = run(d, &["scene.json", "-o", "unmasked"]);
    ok(&unmasked);
    let lines = String::from_utf8(unmasked.stdout).unwrap();
    assert_eq!(
        lines
            .lines()
            .filter(|line| line.starts_with("WARN "))
            .count(),
        9
    );
    assert!(lines.contains("warnings=9"));
    fs::write(d.join("scene.json"), SIMPLE).unwrap();
    let safe = run(
        d,
        &[
            "scene.json",
            "-o",
            "safe",
            "--preview",
            "safe_preview",
            "--preview-scale",
            "1",
        ],
    );
    ok(&safe);
    let text = String::from_utf8(safe.stdout).unwrap();
    assert!(!text.contains("WARN"));
    assert!(text.contains("warnings=0"));
    assert!(text.contains("scale=1 grid=false"));
}
#[test]
fn cli_examples() {
    let examples = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/layout-sketch/assets/examples"
    ));
    let mut files: Vec<_> = fs::read_dir(examples)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 6);
    for path in files {
        let dir = tempfile::tempdir().unwrap();
        let input = fs::read(&path).unwrap();
        let source = scene::parse_scene(&input).unwrap();
        let expanded = geometry::expand_scene(&source).unwrap();
        let arg = path.to_str().unwrap();
        ok(&run(dir.path(), &[arg, "-o", "main", "--preview", "prev"]));
        let main = decoded(&dir.path().join("main"));
        let prev = decoded(&dir.path().join("prev"));
        assert_eq!(main, render::render_main(source.canvas, &expanded).unwrap());
        assert_eq!(
            prev,
            image::imageops::resize(
                &main,
                main.width() / 4,
                main.height() / 4,
                FilterType::Lanczos3
            )
        );
        ok(&run(
            dir.path(),
            &[arg, "-o", "main", "--preview", "prev", "--overwrite"],
        ));
        assert_eq!(decoded(&dir.path().join("main")), main);
        assert_eq!(decoded(&dir.path().join("prev")), prev);
        assert_eq!(fs::read(path).unwrap(), input);
    }
}

#[test]
fn cli_backend_scan_range() {
    for shape in [
        serde_json::json!({"type":"rect","x":-6e7,"y":-6e7,"w":1.2e8,"h":1.2e8,"fill":"black"}),
        serde_json::json!({"type":"line","from":[0,0],"to":[1e9,1e9/3.0]}),
    ] {
        for existing in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let d = dir.path();
            let input = serde_json::json!({"schema_version":1,"canvas":"square","shapes":[shape]})
                .to_string();
            fs::write(d.join("scene.json"), &input).unwrap();
            if existing {
                fs::write(d.join("main"), b"main sentinel").unwrap();
                fs::write(d.join("prev"), b"preview sentinel").unwrap();
            }
            let result = run(
                d,
                &[
                    "scene.json",
                    "-o",
                    "main",
                    "--preview",
                    "prev",
                    "--overwrite",
                ],
            );
            assert_eq!(result.status.code(), Some(2));
            let stderr = String::from_utf8(result.stderr).unwrap();
            assert!(stderr.contains("ERROR INVALID_SHAPE path=shapes[0]"));
            assert!(stderr.contains("coordinate exceeds raster backend numeric range"));
            assert!(result.stdout.is_empty());
            assert_eq!(fs::read(d.join("scene.json")).unwrap(), input.as_bytes());
            let mut entries: Vec<_> = fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect();
            entries.sort();
            if existing {
                assert_eq!(fs::read(d.join("main")).unwrap(), b"main sentinel");
                assert_eq!(fs::read(d.join("prev")).unwrap(), b"preview sentinel");
                assert_eq!(entries, ["main", "prev", "scene.json"]);
            } else {
                assert_eq!(entries, ["scene.json"]);
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    fs::write(d.join("scene.json"), r#"{"schema_version":1,"canvas":"square","shapes":[{"type":"line","from":[-10,20],"to":[10,20]}]}"#).unwrap();
    let result = run(d, &["scene.json", "-o", "main", "--preview", "prev"]);
    ok(&result);
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("WARN OUTSIDE")
    );
    assert_eq!(decoded(&d.join("main")).get_pixel(0, 205).0, [0, 0, 0]);
    assert!(decoded(&d.join("prev")).pixels().any(|p| p.0[0] < 128));
}

#[test]
fn cli_backend_extent_limit() {
    let m = 536_870_880.0 / 10.24;
    let diagonal = serde_json::json!({"schema_version":1,"canvas":"square","shapes":[{"type":"line","from":[-m,-m/3.0],"to":[m,m/3.0]}]}).to_string();
    let horizontal = r#"{"schema_version":1,"canvas":"square","shapes":[{"type":"line","from":[9.765625,9.765625],"to":[-52428796.875,9.765625]}]}"#;
    for input in [horizontal, diagonal.as_str()] {
        for existing in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let d = dir.path();
            fs::write(d.join("scene.json"), input).unwrap();
            if existing {
                fs::write(d.join("main"), b"main sentinel").unwrap();
                fs::write(d.join("prev"), b"preview sentinel").unwrap();
            }
            let mut args = vec!["scene.json", "-o", "main", "--preview", "prev"];
            if existing {
                args.push("--overwrite");
            }
            let result = run(d, &args);
            assert_eq!(result.status.code(), Some(2));
            let stderr = String::from_utf8(result.stderr).unwrap();
            assert!(stderr.contains("ERROR INVALID_SHAPE path=shapes[0]"));
            assert!(stderr.contains("path bounds exceed raster precision limit of 16777216 px; reduce the shape size or coordinate magnitudes"));
            assert!(!stderr.contains("panic") && !stderr.contains("backtrace"));
            assert!(result.stdout.is_empty());
            assert_eq!(fs::read(d.join("scene.json")).unwrap(), input.as_bytes());
            let mut entries: Vec<_> = fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect();
            entries.sort();
            if existing {
                assert_eq!(fs::read(d.join("main")).unwrap(), b"main sentinel");
                assert_eq!(fs::read(d.join("prev")).unwrap(), b"preview sentinel");
                assert_eq!(entries, ["main", "prev", "scene.json"]);
            } else {
                assert!(!d.join("main").exists() && !d.join("prev").exists());
                assert_eq!(entries, ["scene.json"]);
            }
        }
    }
}
