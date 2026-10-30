use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run_cli(arguments: &[&str], directory: &tempfile::TempDir) -> Output {
    run_cli_with_streams(arguments, directory, Stdio::piped(), Stdio::piped())
}

fn run_cli_with_streams(
    arguments: &[&str],
    directory: &tempfile::TempDir,
    stdout: Stdio,
    stderr: Stdio,
) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sketch"))
        .args(arguments)
        .current_dir(directory.path())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("sketch exceeded 5-second timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

#[cfg(target_os = "linux")]
fn full_stream() -> Stdio {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    Stdio::from(file)
}

#[cfg(target_os = "linux")]
#[test]
fn cli_contract_stdout_failures() {
    let directory = tempfile::tempdir().unwrap();
    for argument in ["--help", "--version"] {
        let result = run_cli_with_streams(&[argument], &directory, full_stream(), Stdio::piped());
        assert_eq!(result.status.code(), Some(3));
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.starts_with("ERROR output: "), "{error}");
        assert!(!error.contains("panicked"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn cli_contract_broken_pipe() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    let directory = tempfile::tempdir().unwrap();
    for argument in ["--help", "--version"] {
        let (writer, reader) = UnixStream::pair().unwrap();
        drop(reader);
        let stdout = Stdio::from(OwnedFd::from(writer));
        let result = run_cli_with_streams(&[argument], &directory, stdout, Stdio::piped());
        assert_eq!(result.status.code(), Some(3));
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.starts_with("ERROR output: "), "{error}");
        assert!(!error.contains("panicked"), "{error}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn cli_contract_stderr_failures() {
    let directory = tempfile::tempdir().unwrap();
    let result = run_cli_with_streams(&["--unknown"], &directory, Stdio::piped(), full_stream());
    assert_eq!(result.status.code(), Some(3));
    assert!(result.stdout.is_empty());
    let result = run_cli_with_streams(&["--help"], &directory, full_stream(), full_stream());
    assert_eq!(result.status.code(), Some(3));
}
#[test]
fn cli_contract_information() {
    let directory = tempfile::tempdir().unwrap();
    let version = run_cli(&["--version"], &directory);
    assert_eq!(version.status.code(), Some(0));
    assert_eq!(version.stdout, b"sketch 0.1.0 (schema 1)\n");
    assert!(version.stderr.is_empty());
    let help = run_cli(&["--help"], &directory);
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("Usage:") && text.contains("--preview"));
    assert!(!text.contains("OK "));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
#[test]
fn cli_contract_argument_errors_preserve_files() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("scene.json"),
        br#"{"schema_version":1,"shapes":[{"type":"line","from":[1,1],"to":[2,2]}]}"#,
    )
    .unwrap();
    std::fs::write(directory.path().join("out.png"), b"output sentinel").unwrap();
    for arguments in [
        &[][..],
        &["--unknown"],
        &["scene.json", "-o", "out.png", "--unknown"],
        &["--version", "--help"],
    ] {
        let result = run_cli(arguments, &directory);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains("ERROR argument:") && !error.contains("panic"));
        assert_eq!(
            std::fs::read(directory.path().join("scene.json")).unwrap(),
            br#"{"schema_version":1,"shapes":[{"type":"line","from":[1,1],"to":[2,2]}]}"#
        );
        assert_eq!(
            std::fs::read(directory.path().join("out.png")).unwrap(),
            b"output sentinel"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
