use std::process::Command;

#[test]
fn version_flag_prints_installed_command_and_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_media-organizer"))
        .arg("--version")
        .output()
        .expect("run the built executable");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 output"),
        concat!("media-organizer ", env!("CARGO_PKG_VERSION"), "\n"),
    );
}
