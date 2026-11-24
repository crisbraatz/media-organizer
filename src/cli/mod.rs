use std::ffi::OsString;
use std::process::ExitCode;

pub(crate) fn run(mut arguments: impl Iterator<Item = OsString>) -> ExitCode {
    let first = arguments.next();
    if arguments.next().is_some() {
        return usage_error();
    }

    match first.as_deref().and_then(|argument| argument.to_str()) {
        Some("--version" | "-V") => {
            println!("media-organizer {}", env!("CARGO_PKG_VERSION"));
        }
        Some("--help" | "-h") => {
            println!("Usage: media-organizer [--help | --version]");
        }
        None if first.is_none() => {
            println!("Media organization is pending. Run media-organizer --help for usage.");
        }
        _ => return usage_error(),
    }

    ExitCode::SUCCESS
}

fn usage_error() -> ExitCode {
    eprintln!("Invalid arguments. Run media-organizer --help for usage.");
    ExitCode::from(2)
}
