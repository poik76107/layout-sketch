use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use image::{ExtendedColorType, ImageEncoder, RgbImage};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub(crate) enum OutputError {
    Alias(&'static str, &'static str),
    Io {
        operation: &'static str,
        path: PathBuf,
        detail: String,
    },
}
impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(a, b) => write!(f, "ERROR argument: {a} and {b} refer to the same file"),
            Self::Io {
                operation,
                path,
                detail,
            } => write!(
                f,
                "ERROR {operation} path={}: {detail}",
                crate::cli::display_path(path)
            ),
        }
    }
}
fn failure(operation: &'static str, path: &Path, error: impl fmt::Display) -> OutputError {
    OutputError::Io {
        operation,
        path: path.to_owned(),
        detail: error.to_string(),
    }
}
fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}
struct Identity {
    resolved: PathBuf,
    file: Option<fs::Metadata>,
}
fn identity(path: &Path) -> Result<Identity, OutputError> {
    let resolved = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = fs::canonicalize(parent(path)).map_err(|e| failure("output", path, e))?;
            let name = path
                .file_name()
                .ok_or_else(|| failure("output", path, "missing filename"))?;
            parent.join(name)
        }
        Err(error) => return Err(failure("output", path, error)),
    };
    Ok(Identity {
        resolved,
        file: fs::metadata(path).ok(),
    })
}
fn same(a: &Identity, b: &Identity) -> bool {
    if a.resolved == b.resolved {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Some(a), Some(b)) = (&a.file, &b.file) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
}
pub(crate) fn preflight(
    input: &Path,
    main: &Path,
    preview: Option<&Path>,
    overwrite: bool,
) -> Result<(), OutputError> {
    let input_id = identity(input)?;
    let main_id = identity(main)?;
    if same(&input_id, &main_id) {
        return Err(OutputError::Alias("input", "output"));
    }
    if let Some(path) = preview {
        let preview_id = identity(path)?;
        if same(&input_id, &preview_id) {
            return Err(OutputError::Alias("input", "preview"));
        }
        if same(&main_id, &preview_id) {
            return Err(OutputError::Alias("output", "preview"));
        }
    }
    check_target(main, overwrite)?;
    if let Some(path) = preview {
        check_target(path, overwrite)?;
    }
    Ok(())
}
fn check_target(path: &Path, overwrite: bool) -> Result<(), OutputError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            Err(failure("output", path, "destination is a directory"))
        }
        Ok(_) if !overwrite => Err(failure("output", path, "destination already exists")),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure("output", path, error)),
    }
}
pub(crate) struct Staged {
    file: NamedTempFile,
    path: PathBuf,
}
fn encode(writer: &mut impl Write, image: &RgbImage, path: &Path) -> Result<(), OutputError> {
    image::codecs::png::PngEncoder::new(&mut *writer)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| failure("encode", path, e))?;
    writer.flush().map_err(|e| failure("flush", path, e))
}
pub(crate) fn stage(path: &Path, image: &RgbImage) -> Result<Staged, OutputError> {
    stage_using(path, image, encode)
}
fn stage_using(
    path: &Path,
    image: &RgbImage,
    encoder: impl FnOnce(&mut fs::File, &RgbImage, &Path) -> Result<(), OutputError>,
) -> Result<Staged, OutputError> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut file = builder
        .tempfile_in(parent(path))
        .map_err(|e| failure("stage", path, e))?;
    encoder(file.as_file_mut(), image, path)?;
    Ok(Staged {
        file,
        path: path.to_owned(),
    })
}

#[cfg(test)]
mod tests;
fn publish(file: Staged, overwrite: bool) -> Result<(), OutputError> {
    if overwrite {
        file.file
            .persist(&file.path)
            .map_err(|e| failure("publish", &file.path, e.error))?;
    } else {
        fs::hard_link(file.file.path(), &file.path)
            .map_err(|e| failure("publish", &file.path, e))?;
        file.file
            .close()
            .map_err(|e| failure("cleanup", &file.path, e))?;
    }
    Ok(())
}
pub(crate) fn publish_pair(
    main: Staged,
    preview: Option<Staged>,
    overwrite: bool,
) -> Result<(), OutputError> {
    if let Some(preview) = preview {
        publish(preview, overwrite)?;
    }
    publish(main, overwrite)
}

#[cfg(test)]
mod release_checks;
