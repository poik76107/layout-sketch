use std::ffi::OsString;
use std::fmt::{self, Arguments};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::{bounds, geometry, output, render, scene};

const HELP: &str = "sketch: render a JSON scene to PNG (schema 1)\n\nUsage: sketch scene.json -o main.png [--preview preview.png] [--preview-scale S] [--grid] [--overwrite]\n       sketch --help | --version\n\n-o PATH, --output PATH   Main PNG (required)\n--preview PATH           Optional preview PNG\n--preview-scale S        Preview scale 0.1 to 1 (default 0.25); requires --preview\n--grid                   Grid on preview only; requires --preview\n--overwrite              Replace existing outputs\n\nPrimitive  Geometry\nline       Segment by endpoints or angle and length\npolyline   Open chain of points\npolygon    Closed contour of points\nrect       Rectangle\ncircle     Circle by center and radius\nellipse    Rotated ellipse\narc        Elliptical arc, optionally closed by a chord\ncurve      Cubic Bezier\npath       M/L/Q/C/Z path data\n\nGroups and repeat_rotate combine primitives. Scene coordinates use 100 units\non the short canvas side, with origin at top left and Y downward. Thick and\nthin strokes are 9 and 4.5 pixels, with round caps and joins.\nPainted paths after transforms and stroke expansion have outward-rounded bounds\nof at most 16777216 pixels per axis, inclusive. This checks stroke outlines and\nsource paths with white or black fill; allow_clip does not bypass the limit.";
struct Options {
    input: PathBuf,
    output: PathBuf,
    preview: Option<PathBuf>,
    scale: f64,
    grid: bool,
    overwrite: bool,
}
enum Command {
    Information(&'static str),
    Render(Options),
}

pub fn run_cli(arguments: impl IntoIterator<Item = OsString>) -> u8 {
    match parse_arguments(arguments) {
        Ok(Command::Information(text)) => write_information(text),
        Ok(Command::Render(options)) => match execute(&options, &mut io::stdout().lock()) {
            Ok(()) => 0,
            Err(error) => report_error(format_args!("{error}"), error.code()),
        },
        Err(message) => report_error(format_args!("ERROR argument: {message}"), 2),
    }
}
fn take_value(
    arguments: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<OsString, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("missing value for {name}"))?;
    if value.to_str().is_some_and(|s| s.starts_with('-')) {
        return Err(format!("missing value for {name}"));
    }
    Ok(value)
}
fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut arguments = arguments.into_iter().peekable();
    if let Some(first) = arguments.peek().and_then(|s| s.to_str())
        && (first == "--help" || first == "--version")
    {
        let text = if first == "--help" {
            HELP
        } else {
            "sketch 0.1.0 (schema 1)"
        };
        arguments.next();
        if arguments.next().is_some() {
            return Err("--help and --version must be standalone".into());
        }
        return Ok(Command::Information(text));
    }
    let mut input = None;
    let mut output = None;
    let mut preview = None;
    let mut scale = None;
    let mut grid = false;
    let mut overwrite = false;
    while let Some(arg) = arguments.next() {
        match arg.to_str() {
            Some("-o" | "--output") => {
                if output.is_some() {
                    return Err("duplicate --output".into());
                }
                output = Some(PathBuf::from(take_value(&mut arguments, "--output")?));
            }
            Some("--preview") => {
                if preview.is_some() {
                    return Err("duplicate --preview".into());
                }
                preview = Some(PathBuf::from(take_value(&mut arguments, "--preview")?));
            }
            Some("--preview-scale") => {
                if scale.is_some() {
                    return Err("duplicate --preview-scale".into());
                }
                let value = take_value(&mut arguments, "--preview-scale")?;
                let number = value
                    .to_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or("invalid --preview-scale")?;
                if !number.is_finite() || !(0.1..=1.0).contains(&number) {
                    return Err("invalid --preview-scale (expected 0.1 to 1)".into());
                }
                scale = Some(number);
            }
            Some("--grid") => {
                if grid {
                    return Err("duplicate --grid".into());
                }
                grid = true;
            }
            Some("--overwrite") => {
                if overwrite {
                    return Err("duplicate --overwrite".into());
                }
                overwrite = true;
            }
            Some(value) if value.starts_with('-') => return Err(format!("unknown option {value}")),
            _ => {
                if input.is_some() {
                    return Err("extra input path".into());
                }
                input = Some(PathBuf::from(arg));
            }
        }
    }
    let input = input.ok_or("missing input path")?;
    let output = output.ok_or("missing --output")?;
    if preview.is_none() && (grid || scale.is_some()) {
        return Err("--grid and --preview-scale require --preview".into());
    }
    Ok(Command::Render(Options {
        input,
        output,
        preview,
        scale: scale.unwrap_or(0.25),
        grid,
        overwrite,
    }))
}
#[derive(Debug)]
enum CliError {
    Scene(scene::SceneError),
    Render(render::RenderError),
    Output(output::OutputError),
    Io(String),
}
impl CliError {
    fn code(&self) -> u8 {
        match self {
            Self::Scene(_)
            | Self::Render(render::RenderError::InvalidGeometry(_))
            | Self::Output(output::OutputError::Alias(..)) => 2,
            _ => 3,
        }
    }
}
impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(e) => write!(f, "{e}"),
            Self::Render(render::RenderError::InvalidGeometry(e)) => write!(f, "{e}"),
            Self::Render(e) => write!(f, "ERROR {e}"),
            Self::Output(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "ERROR {e}"),
        }
    }
}
pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '\\')
    {
        return serde_json::to_string(&*text).unwrap_or_else(|_| "\"invalid path\"".into());
    }
    text.into_owned()
}
fn execute(options: &Options, stdout: &mut impl Write) -> Result<(), CliError> {
    let file = File::open(&options.input)
        .map_err(|e| CliError::Io(format!("input path={}: {e}", display_path(&options.input))))?;
    let mut bytes = Vec::new();
    file.take(scene::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| CliError::Io(format!("input path={}: {e}", display_path(&options.input))))?;
    let scene = scene::parse_scene(&bytes).map_err(CliError::Scene)?;
    let primitives = geometry::expand_scene(&scene).map_err(CliError::Scene)?;
    let warnings = bounds::check_bounds(scene.canvas, &primitives).map_err(CliError::Render)?;
    output::preflight(
        &options.input,
        &options.output,
        options.preview.as_deref(),
        options.overwrite,
    )
    .map_err(CliError::Output)?;
    let main = render::render_main(scene.canvas, &primitives).map_err(CliError::Render)?;
    let preview = if options.preview.is_some() {
        Some(render::render_preview(&main, options.scale, options.grid).map_err(CliError::Render)?)
    } else {
        None
    };
    let staged_main = output::stage(&options.output, &main).map_err(CliError::Output)?;
    let staged_preview = match (&options.preview, &preview) {
        (Some(path), Some(image)) => Some(output::stage(path, image).map_err(CliError::Output)?),
        _ => None,
    };
    for warning in &warnings {
        writeln!(stdout, "{warning}").map_err(|e| CliError::Io(format!("output: {e}")))?;
    }
    stdout
        .flush()
        .map_err(|e| CliError::Io(format!("output: {e}")))?;
    output::publish_pair(staged_main, staged_preview, options.overwrite)
        .map_err(CliError::Output)?;
    writeln!(
        stdout,
        "OK output={} size={}x{} warnings={}",
        display_path(&options.output),
        main.width(),
        main.height(),
        warnings.len()
    )
    .map_err(|e| CliError::Io(format!("output: {e}")))?;
    if let (Some(path), Some(image)) = (&options.preview, preview) {
        writeln!(
            stdout,
            "OK preview={} size={}x{} scale={} grid={}",
            display_path(path),
            image.width(),
            image.height(),
            options.scale,
            options.grid
        )
        .map_err(|e| CliError::Io(format!("output: {e}")))?;
    }
    stdout
        .flush()
        .map_err(|e| CliError::Io(format!("output: {e}")))
}
fn write_information(text: &str) -> u8 {
    let mut output = io::stdout().lock();
    if let Err(error) = writeln!(output, "{text}") {
        return report_error(format_args!("ERROR output: {error}"), 3);
    }
    if let Err(error) = output.flush() {
        return report_error(format_args!("ERROR output: {error}"), 3);
    }
    0
}
fn report_error(message: Arguments<'_>, exit_code: u8) -> u8 {
    let mut output = io::stderr().lock();
    // If stderr also fails, only the I/O exit code can report the failure.
    if writeln!(output, "{message}").is_err() || output.flush().is_err() {
        return 3;
    }
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FailingWriter {
        bytes: Vec<u8>,
        fail_write: bool,
        fail_flush: bool,
        short: bool,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                return Err(io::Error::other("write failure"));
            }
            let n = if self.short {
                bytes.len().min(3)
            } else {
                bytes.len()
            };
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::other("flush failure"))
            } else {
                Ok(())
            }
        }
    }
    struct LateFailure {
        bytes: Vec<u8>,
        flushes: usize,
        fail_flush: bool,
    }
    impl Write for LateFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.fail_flush && self.bytes.len() >= 3 {
                return Err(io::Error::other("late write"));
            }
            let size = if self.fail_flush {
                bytes.len()
            } else {
                bytes.len().min(3 - self.bytes.len())
            };
            self.bytes.extend_from_slice(&bytes[..size]);
            Ok(size)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.fail_flush && self.flushes == 2 {
                return Err(io::Error::other("late flush"));
            }
            Ok(())
        }
    }
    #[test]
    fn cli_error_reporting() {
        let scene_error = scene::parse_scene(b"{").unwrap_err();
        let shape_error = scene::parse_scene(
            br#"{"schema_version":1,"shapes":[{"type":"line","from":[0,0],"length":0,"angle":0}]}"#,
        )
        .unwrap_err();
        let errors = [
            CliError::Scene(scene_error),
            CliError::Render(render::RenderError::InvalidGeometry(shape_error)),
            CliError::Render(render::RenderError::Raster {
                source_path: Some("shapes[1]".into()),
                repeat_index: Some(2),
                detail: "raster fail".into(),
            }),
            CliError::Output(output::OutputError::Alias("input", "output")),
            CliError::Output(output::OutputError::Io {
                operation: "encode",
                path: "out".into(),
                detail: "failed".into(),
            }),
            CliError::Output(output::OutputError::Io {
                operation: "publish",
                path: "out".into(),
                detail: "failed".into(),
            }),
            CliError::Io("input: failed".into()),
        ];
        for (index, error) in errors.iter().enumerate() {
            assert_eq!(error.code(), if matches!(index, 0 | 1 | 3) { 2 } else { 3 });
            assert!(error.to_string().starts_with("ERROR "));
            assert!(!error.to_string().contains("OK "));
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input");
        std::fs::write(
            &input,
            br#"{"schema_version":1,"shapes":[{"type":"line","from":[0,20],"to":[0,80]}]}"#,
        )
        .unwrap();
        for (fail_write, fail_flush, short) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let target = dir.path().join("out");
            let options = Options {
                input: input.clone(),
                output: target.clone(),
                preview: None,
                scale: 0.25,
                grid: false,
                overwrite: true,
            };
            let mut writer = FailingWriter {
                bytes: Vec::new(),
                fail_write,
                fail_flush,
                short,
            };
            let result = execute(&options, &mut writer);
            if short {
                assert!(result.is_ok());
                assert!(target.exists());
            } else {
                assert_eq!(result.unwrap_err().code(), 3);
                assert!(!target.exists());
            }
        }
        std::fs::write(
            &input,
            br#"{"schema_version":1,"shapes":[{"type":"line","from":[20,20],"to":[80,20]}]}"#,
        )
        .unwrap();
        for fail_flush in [false, true] {
            let target = dir.path().join("late");
            let options = Options {
                input: input.clone(),
                output: target.clone(),
                preview: None,
                scale: 0.25,
                grid: false,
                overwrite: true,
            };
            let mut writer = LateFailure {
                bytes: Vec::new(),
                flushes: 0,
                fail_flush,
            };
            assert_eq!(execute(&options, &mut writer).unwrap_err().code(), 3);
            assert!(target.exists());
            assert!(!writer.bytes.is_empty());
        }
    }
}
