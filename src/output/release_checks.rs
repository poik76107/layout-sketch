use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{encode, publish_pair, stage};
use crate::{bounds, geometry, render, scene};

fn binary() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("SKETCH_RELEASE_BINARY").expect("SKETCH_RELEASE_BINARY must be set"),
    );
    assert!(
        path.is_absolute() && path.is_file(),
        "SKETCH_RELEASE_BINARY must name an existing absolute executable"
    );
    path
}
fn examples() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/layout-sketch/assets/examples");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 6);
    paths
}
fn run(binary: &Path, cwd: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(binary)
        .current_dir(cwd)
        .env("PATH", "")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("release executable timed out after 30 seconds");
        }
        thread::sleep(Duration::from_millis(1));
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("OK output="));
}
fn rgb(path: &Path) -> image::RgbImage {
    let img = image::open(path).unwrap();
    assert_eq!(img.color(), image::ColorType::Rgb8);
    img.into_rgb8()
}

#[test]
#[ignore = "requires an assembled release executable and retains external visual evidence"]
fn release_examples() {
    let bin = binary();
    let root = tempfile::Builder::new()
        .prefix("sketch release visual ")
        .tempdir()
        .unwrap()
        .keep();
    println!("VISUAL_DIR={}", root.display());
    for (i, path) in examples().iter().enumerate() {
        let before = fs::read(path).unwrap();
        let scene = scene::parse_scene(&before).unwrap();
        let primitives = geometry::expand_scene(&scene).unwrap();
        let expected = render::render_main(scene.canvas, &primitives).unwrap();
        let preview = render::render_preview(&expected, 0.25, false).unwrap();
        let main_name = format!("{i}-main.png");
        let preview_name = format!("{i}-preview.png");
        let input = path.to_str().unwrap();
        let output = run(
            &bin,
            &root,
            &[input, "-o", &main_name, "--preview", &preview_name],
        );
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("OK preview="));
        assert_eq!(rgb(&root.join(&main_name)), expected);
        assert_eq!(rgb(&root.join(&preview_name)), preview);
        let main_before = fs::read(root.join(&main_name)).unwrap();
        let preview_before = fs::read(root.join(&preview_name)).unwrap();
        success(&run(
            &bin,
            &root,
            &[
                input,
                "-o",
                &main_name,
                "--preview",
                &preview_name,
                "--overwrite",
            ],
        ));
        assert_eq!(fs::read(root.join(&main_name)).unwrap(), main_before);
        assert_eq!(fs::read(root.join(&preview_name)).unwrap(), preview_before);
        assert_eq!(fs::read(path).unwrap(), before);
        println!(
            "{}: {} {:?} {} {:?}",
            path.display(),
            root.join(&main_name).display(),
            expected.dimensions(),
            root.join(&preview_name).display(),
            preview.dimensions()
        );
        let hashes = Command::new("sha256sum")
            .arg(root.join(&main_name))
            .arg(root.join(&preview_name))
            .output()
            .unwrap();
        assert!(hashes.status.success());
        print!("{}", String::from_utf8(hashes.stdout).unwrap());
    }
    let valid = examples()[0].clone();
    let cases = [
        ("malformed", "{", 2),
        (
            "path",
            r#"{"schema_version":1,"canvas":"square","shapes":[{"id":"a","type":"path","d":"M 0 0 H 1"}]}"#,
            2,
        ),
        (
            "duplicate",
            r#"{"schema_version":1,"canvas":"square","shapes":[{"id":"a","type":"circle","cx":10,"cy":10,"r":1},{"id":"a","type":"circle","cx":20,"cy":20,"r":1}]}"#,
            2,
        ),
    ];
    for (name, body, status) in cases {
        let path = root.join(format!("{name}.json"));
        fs::write(&path, body).unwrap();
        let result = run(&bin, &root, &[path.to_str().unwrap(), "-o", "bad.png"]);
        assert_eq!(result.status.code(), Some(status));
        assert!(!String::from_utf8_lossy(&result.stdout).contains("OK"));
        assert!(!root.join("bad.png").exists());
    }
    fs::write(root.join("sentinel"), b"unchanged").unwrap();
    for dest in ["sentinel", "missing-parent/out.png"] {
        let result = run(&bin, &root, &[valid.to_str().unwrap(), "-o", dest]);
        assert_eq!(result.status.code(), Some(3));
        assert!(!String::from_utf8_lossy(&result.stdout).contains("OK"));
        assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"unchanged");
    }
}

#[test]
#[ignore = "requires an assembled release executable and measures real release boundaries"]
fn release_profile() {
    let bin = binary();
    let dir = tempfile::Builder::new()
        .prefix("sketch release timings ")
        .tempdir()
        .unwrap()
        .keep();
    let raw_path = dir.join("samples.csv");
    let mut raw = String::from(
        "scene,mode,iteration,prep_ms,main_ms,preview_ms,encode_main_ms,encode_preview_ms,stage_ms,publish_ms,full_ms\n",
    );
    println!("TIMING_DIR={}", dir.display());
    for path in examples() {
        let bytes = fs::read(&path).unwrap();
        for (mode, scale, grid) in [
            ("main", None, false),
            ("preview", Some(0.25), false),
            ("half", Some(0.5), false),
            ("grid", Some(0.25), true),
        ] {
            for iteration in 0..23 {
                let t = Instant::now();
                let scene = scene::parse_scene(black_box(&bytes)).unwrap();
                let primitives = geometry::expand_scene(&scene).unwrap();
                black_box(bounds::check_bounds(scene.canvas, &primitives).unwrap());
                let prep = t.elapsed().as_secs_f64() * 1000.0;
                let t = Instant::now();
                let main = black_box(render::render_main(scene.canvas, &primitives).unwrap());
                let main_ms = t.elapsed().as_secs_f64() * 1000.0;
                let t = Instant::now();
                let preview =
                    scale.map(|s| black_box(render::render_preview(&main, s, grid).unwrap()));
                let preview_ms = scale.map(|_| t.elapsed().as_secs_f64() * 1000.0);
                let main_path = dir.join("main.png");
                let prev_path = dir.join("preview.png");
                let t = Instant::now();
                let mut buffer = Vec::new();
                encode(&mut buffer, &main, &main_path).unwrap();
                black_box(buffer);
                let encode_main = t.elapsed().as_secs_f64() * 1000.0;
                let t = Instant::now();
                let encode_preview = preview.as_ref().map(|img| {
                    let mut buffer = Vec::new();
                    encode(&mut buffer, img, &prev_path).unwrap();
                    black_box(buffer);
                    t.elapsed().as_secs_f64() * 1000.0
                });
                let t = Instant::now();
                let staged_main = stage(&main_path, &main).unwrap();
                let staged_preview = preview.as_ref().map(|img| stage(&prev_path, img).unwrap());
                let stage_ms = t.elapsed().as_secs_f64() * 1000.0;
                let t = Instant::now();
                publish_pair(staged_main, staged_preview, true).unwrap();
                let publish_ms = t.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(rgb(&main_path), main);
                if let Some(img) = &preview {
                    assert_eq!(&rgb(&prev_path), img);
                }
                let full_main = dir.join("full-main.png");
                let full_preview = dir.join("full-preview.png");
                let mut args = vec![
                    path.to_str().unwrap(),
                    "-o",
                    full_main.to_str().unwrap(),
                    "--overwrite",
                ];
                if scale.is_some() {
                    args.extend(["--preview", full_preview.to_str().unwrap()]);
                }
                if scale == Some(0.5) {
                    args.extend(["--preview-scale", "0.5"]);
                }
                if grid {
                    args.push("--grid");
                }
                let t = Instant::now();
                let output = run(&bin, &dir, &args);
                let full_ms = t.elapsed().as_secs_f64() * 1000.0;
                success(&output);
                assert_eq!(rgb(&full_main), main);
                if let Some(img) = &preview {
                    assert_eq!(&rgb(&full_preview), img);
                }
                let opt =
                    |n: Option<f64>| n.map_or_else(|| "N/A".to_string(), |v| format!("{v:.6}"));
                raw.push_str(&format!("{},{mode},{iteration},{prep:.6},{main_ms:.6},{},{encode_main:.6},{},{stage_ms:.6},{publish_ms:.6},{full_ms:.6}\n", path.file_name().unwrap().to_str().unwrap(), opt(preview_ms), opt(encode_preview)));
            }
            fs::write(&raw_path, &raw).unwrap();
        }
    }
    println!("SAMPLES={}", raw_path.display());
}
