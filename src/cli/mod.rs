use crate::application::{self, MediaSource, PreviewRow};
use crate::domain::{AnalysisReport, MediaCounts};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn run(
    mut arguments: impl Iterator<Item = OsString>,
    source: &impl MediaSource,
) -> ExitCode {
    let mut first = arguments.next();
    let preview = first
        .as_deref()
        .is_some_and(|argument| argument == "--preview");
    if preview {
        first = arguments.next();
    }
    let positional = if first.as_deref().is_some_and(|argument| argument == "--") {
        let Some(path) = arguments.next() else {
            return usage_error();
        };
        Some(path)
    } else {
        match first.as_deref().and_then(|argument| argument.to_str()) {
            Some("--version" | "-V") if !preview && arguments.next().is_none() => {
                println!("media-organizer {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some("--help" | "-h") if !preview && arguments.next().is_none() => {
                println!("Usage: media-organizer [--preview] [[--] <folder>]");
                println!("       media-organizer --help | --version");
                return ExitCode::SUCCESS;
            }
            _ if first
                .as_deref()
                .is_some_and(|argument| argument.as_encoded_bytes().starts_with(b"-")) =>
            {
                return usage_error();
            }
            _ => first,
        }
    };
    if arguments.next().is_some() {
        return usage_error();
    }

    let folder = match positional
        .map(PathBuf::from)
        .map_or_else(prompt_folder, |path| Ok(Some(path)))
    {
        Ok(Some(path)) => path,
        Ok(None) => {
            eprintln!("Folder path is required.");
            return ExitCode::from(2);
        }
        Err(error) => {
            eprintln!("Could not read folder path: {error}");
            return ExitCode::FAILURE;
        }
    };

    match application::analyze_folder(source, &folder, preview) {
        Ok(analysis) => {
            print_report(&analysis.report);
            if let Some(rows) = analysis.preview {
                print_preview(&folder, &rows);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Analysis failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn prompt_folder() -> io::Result<Option<PathBuf>> {
    {
        let mut output = io::stdout().lock();
        write!(output, "Folder path: ")?;
        output.flush()?;
    }

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let path = line.strip_suffix('\n').map_or(line.as_str(), |line| {
        line.strip_suffix('\r').unwrap_or(line)
    });
    Ok((!path.trim().is_empty()).then(|| PathBuf::from(path)))
}

fn print_report(report: &AnalysisReport) {
    print_counts("Media", &report.total());
    print_counts("Photos", &report.photos);
    print_counts("Videos", &report.videos);
    println!("Subfolders: {}", report.subfolders);
    for (extension, counts) in &report.extensions {
        print_counts(&format!(".{extension}"), counts);
    }
}

fn print_preview(root: &Path, rows: &[PreviewRow]) {
    println!("Rename preview:");
    if rows.is_empty() {
        println!("No media files.");
    }
    for row in rows {
        let source = relative_to(root, &row.name.source);
        let destination = relative_to(root, &row.name.destination);
        let status = if row.blocked {
            " [blocked]"
        } else if source == destination {
            " [unchanged]"
        } else {
            ""
        };
        println!("{source:?} -> {destination:?}{status}");
    }
}

fn relative_to<'a>(root: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

fn print_counts(label: &str, counts: &MediaCounts) {
    println!(
        "{label}: {} (duplicates: {})",
        counts.files, counts.duplicates
    );
}

fn usage_error() -> ExitCode {
    eprintln!("Invalid arguments. Run media-organizer --help for usage.");
    ExitCode::from(2)
}
