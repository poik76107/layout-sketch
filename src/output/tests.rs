use super::*;
use image::Rgb;
use std::io::Error;

fn img() -> RgbImage {
    RgbImage::from_pixel(4, 3, Rgb([2, 3, 4]))
}
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
#[test]
fn output_noclobber_race() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main");
    let staged = stage(&path, &img()).unwrap();
    fs::write(&path, b"competitor").unwrap();
    assert!(matches!(
        publish_pair(staged, None, false),
        Err(OutputError::Io {
            operation: "publish",
            ..
        })
    ));
    assert_eq!(fs::read(&path).unwrap(), b"competitor");
    assert_eq!(names(dir.path()), ["main"]);
    fs::remove_file(&path).unwrap();
    #[cfg(unix)]
    {
        let staged = stage(&path, &img()).unwrap();
        std::os::unix::fs::symlink("other", &path).unwrap();
        assert!(publish_pair(staged, None, false).is_err());
        assert_eq!(fs::read_link(&path).unwrap(), Path::new("other"));
        fs::remove_file(&path).unwrap();
    }
    publish_pair(stage(&path, &img()).unwrap(), None, false).unwrap();
    assert_eq!(
        image::load_from_memory(&fs::read(&path).unwrap())
            .unwrap()
            .to_rgb8(),
        img()
    );
    assert_eq!(names(dir.path()), ["main"]);
}
struct Fault<'a> {
    file: &'a mut fs::File,
    remaining: usize,
    mode: u8,
    seen: usize,
}
impl Write for Fault<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.seen += bytes.len();
        if self.remaining == 0 {
            return if self.mode == 0 {
                Ok(0)
            } else {
                Err(Error::other("injected write"))
            };
        }
        let length =
            bytes
                .len()
                .min(self.remaining)
                .min(if self.mode == 3 { 3 } else { usize::MAX });
        let count = self.file.write(&bytes[..length])?;
        if self.mode != 3 {
            self.remaining -= count;
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.mode == 2 {
            Err(Error::other("injected flush"))
        } else {
            self.file.flush()
        }
    }
}
#[test]
fn output_staging_failures() {
    for mode in 0..4 {
        for second in [false, true] {
            for existing in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let main = dir.path().join("main");
                let preview = dir.path().join("preview");
                if existing {
                    fs::write(&main, b"sentinel main").unwrap();
                    fs::write(&preview, b"sentinel preview").unwrap();
                }
                let first = if second {
                    Some(stage(&main, &img()).unwrap())
                } else {
                    None
                };
                let target = if second { &preview } else { &main };
                let staged = stage_using(target, &img(), |file, image, path| {
                    let mut writer = Fault {
                        file,
                        remaining: if mode == 0 {
                            0
                        } else if mode == 1 {
                            12
                        } else {
                            usize::MAX
                        },
                        mode,
                        seen: 0,
                    };
                    let result = encode(&mut writer, image, path);
                    assert!(writer.seen > 0);
                    result
                });
                if mode == 3 {
                    assert!(staged.is_ok());
                } else {
                    let Err(OutputError::Io {
                        operation, path, ..
                    }) = &staged
                    else {
                        panic!("expected contextual staging error")
                    };
                    assert_eq!(path, target);
                    assert_eq!(*operation, if mode == 2 { "flush" } else { "encode" });
                }
                drop(staged);
                drop(first);
                if existing {
                    assert_eq!(fs::read(&main).unwrap(), b"sentinel main");
                    assert_eq!(fs::read(&preview).unwrap(), b"sentinel preview");
                    assert_eq!(names(dir.path()), ["main", "preview"]);
                } else {
                    assert!(names(dir.path()).is_empty());
                }
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("short");
    let staged = stage_using(&path, &img(), |file, image, path| {
        let mut writer = Fault {
            file,
            remaining: usize::MAX,
            mode: 3,
            seen: 0,
        };
        encode(&mut writer, image, path)
    })
    .unwrap();
    publish_pair(staged, None, false).unwrap();
    assert_eq!(
        image::load_from_memory(&fs::read(path).unwrap())
            .unwrap()
            .to_rgb8(),
        img()
    );
}
#[test]
fn output_pair_order() {
    for block in ["preview", "main", "none"] {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        let preview = dir.path().join("preview");
        let m = stage(&main, &img()).unwrap();
        let p = stage(&preview, &img()).unwrap();
        if block != "none" {
            fs::write(dir.path().join(block), b"competitor").unwrap();
        }
        let result = publish_pair(m, Some(p), false);
        assert_eq!(result.is_ok(), block == "none");
        if block == "preview" {
            assert!(!main.exists());
            assert_eq!(fs::read(&preview).unwrap(), b"competitor");
        } else {
            assert_eq!(
                image::load_from_memory(&fs::read(&preview).unwrap())
                    .unwrap()
                    .to_rgb8(),
                img()
            );
        }
        if block == "main" {
            assert_eq!(fs::read(&main).unwrap(), b"competitor");
        }
        if block == "none" {
            assert_eq!(
                image::load_from_memory(&fs::read(&main).unwrap())
                    .unwrap()
                    .to_rgb8(),
                img()
            );
        }
        assert!(!names(dir.path()).iter().any(|n| n.starts_with(".tmp")));
    }
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    publish_pair(stage(&main, &img()).unwrap(), None, false).unwrap();
    assert_eq!(
        image::load_from_memory(&fs::read(main).unwrap())
            .unwrap()
            .to_rgb8(),
        img()
    );

    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    let preview = dir.path().join("preview");
    fs::write(&main, b"old main").unwrap();
    fs::write(&preview, b"old preview").unwrap();
    publish_pair(
        stage(&main, &img()).unwrap(),
        Some(stage(&preview, &img()).unwrap()),
        true,
    )
    .unwrap();
    for path in [&main, &preview] {
        assert_eq!(
            image::load_from_memory(&fs::read(path).unwrap())
                .unwrap()
                .to_rgb8(),
            img()
        );
    }
    assert_eq!(names(dir.path()), ["main", "preview"]);

    fs::write(&preview, b"old preview again").unwrap();
    let staged_main = stage(&main, &img()).unwrap();
    let staged_preview = stage(&preview, &img()).unwrap();
    fs::remove_file(&main).unwrap();
    fs::create_dir(&main).unwrap();
    fs::write(main.join("sentinel"), b"keep").unwrap();
    let error = publish_pair(staged_main, Some(staged_preview), true).unwrap_err();
    assert!(
        matches!(error, OutputError::Io { operation: "publish", ref path, .. } if path == &main)
    );
    assert_eq!(fs::read(main.join("sentinel")).unwrap(), b"keep");
    assert_eq!(
        image::load_from_memory(&fs::read(&preview).unwrap())
            .unwrap()
            .to_rgb8(),
        img()
    );
    assert_eq!(names(dir.path()), ["main", "preview"]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        fs::write(&target, b"referent").unwrap();
        symlink("target", &link).unwrap();
        publish_pair(stage(&link, &img()).unwrap(), None, true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"referent");
        assert!(
            !fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            image::load_from_memory(&fs::read(&link).unwrap())
                .unwrap()
                .to_rgb8(),
            img()
        );
        assert_eq!(names(dir.path()), ["link", "target"]);
    }
}
