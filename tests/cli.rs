use std::process::Command;

#[test]
fn version_flag_prints_installed_command_and_version() {
    let output = command()
        .arg("--version")
        .output()
        .expect("run the built executable");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 output"),
        concat!("media-organizer ", env!("CARGO_PKG_VERSION"), "\n"),
    );
}

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "media organizer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_media-organizer"))
}

fn prompt(input: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn analysis_reports_recursive_media_without_changing_the_files() {
    use std::fs;

    let root = Scratch::new();
    let subfolder = root.0.join("nested");
    fs::create_dir(&subfolder).unwrap();
    let files = [
        (root.0.join("a.JPG"), b"same"),
        (subfolder.join("b.png"), b"same"),
        (subfolder.join("c.mov"), b"same"),
        (root.0.join("notes.txt"), b"note"),
    ];
    for (path, bytes) in &files {
        fs::write(path, bytes).unwrap();
    }

    let output = prompt(&format!("{}\r\n", root.0.display()));
    assert!(output.status.success(), "{:?}", output.stderr);
    let report = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Folder path:",
        "Media: 3 (duplicates: 2)",
        "Photos: 2 (duplicates: 1)",
        "Videos: 1 (duplicates: 1)",
        "Subfolders: 1",
        ".jpg: 1 (duplicates: 0)",
        ".mov: 1 (duplicates: 1)",
        ".png: 1 (duplicates: 1)",
    ] {
        assert!(
            report.contains(expected),
            "missing {expected:?} in {report:?}"
        );
    }
    assert!(!report.contains(".txt"));

    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 3);
    assert_eq!(fs::read_dir(&subfolder).unwrap().count(), 2);
    for (path, expected) in files {
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}

#[test]
fn empty_folder_via_argument_reports_zeros() {
    let root = Scratch::new();
    let output = command().arg(&root.0).output().unwrap();
    assert!(output.status.success());
    let report = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Media: 0 (duplicates: 0)",
        "Photos: 0 (duplicates: 0)",
        "Videos: 0 (duplicates: 0)",
        "Subfolders: 0",
    ] {
        assert!(
            report.contains(expected),
            "missing {expected:?} in {report:?}"
        );
    }
    assert!(!report.contains("Folder path:"));
}

#[test]
fn blank_and_missing_paths_report_errors() {
    for input in ["", "\n", " \t\n"] {
        let blank = prompt(input);
        assert_eq!(blank.status.code(), Some(2));
        assert!(
            String::from_utf8(blank.stderr)
                .unwrap()
                .contains("Folder path is required")
        );
    }

    let root = Scratch::new();
    let missing = command().arg(root.0.join("missing")).output().unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let error = String::from_utf8(missing.stderr).unwrap();
    assert!(error.contains("could not read folder") && error.contains("missing"));
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_argument_is_preserved() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = Scratch::new();
    let native = root.0.join(OsStr::from_bytes(b"non-utf8-\xff"));
    std::fs::create_dir(&native).unwrap();
    assert!(command().arg(native).output().unwrap().status.success());
}

#[cfg(unix)]
#[test]
fn literal_prompt_whitespace_is_preserved() {
    let root = Scratch::new();
    let space = root.0.join("trailing ");
    std::fs::create_dir(&space).unwrap();
    assert!(prompt(&format!("{}\n", space.display())).status.success());
}
