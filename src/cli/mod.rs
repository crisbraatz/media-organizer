use crate::application::{self, MediaSource, PreviewRow, RenameSource, apply};
use crate::domain::{AnalysisReport, MediaCounts};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn run(
    mut arguments: impl Iterator<Item = OsString>,
    source: &(impl MediaSource + RenameSource),
) -> ExitCode {
    let mut first = arguments.next();
    let apply = first
        .as_deref()
        .is_some_and(|argument| argument == "--apply");
    let preview = first
        .as_deref()
        .is_some_and(|argument| argument == "--preview");
    if preview || apply {
        first = arguments.next();
    }
    let positional = if first.as_deref().is_some_and(|argument| argument == "--") {
        let Some(path) = arguments.next() else {
            return usage_error();
        };
        Some(path)
    } else {
        match first.as_deref().and_then(|argument| argument.to_str()) {
            Some("--version" | "-V") if !preview && !apply && arguments.next().is_none() => {
                println!("media-organizer {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some("--help" | "-h") if !preview && !apply && arguments.next().is_none() => {
                println!("Usage: media-organizer [--preview | --apply] [[--] <folder>]");
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

    if apply {
        return apply_folder(source, &folder);
    }

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

fn apply_folder(source: &(impl MediaSource + RenameSource), folder: &Path) -> ExitCode {
    let plan = match apply::prepare_apply(source, folder) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("Analysis failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    print_report(&plan.report);
    print_preview(&plan.root, &plan.rows);
    if plan.rows.iter().any(|row| row.blocked) {
        eprintln!("Cannot apply: the preview has occupied destinations.");
        return ExitCode::FAILURE;
    }
    if plan
        .rows
        .iter()
        .all(|row| row.name.source == row.name.destination)
    {
        println!("No files need renaming.");
        return ExitCode::SUCCESS;
    }

    match confirm_apply() {
        Ok(false) => {
            println!("Cancelled. No files were renamed.");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("Could not read confirmation: {error}");
            return ExitCode::FAILURE;
        }
        Ok(true) => {}
    }
    if let Err(error) = apply::revalidate(source, folder, &plan) {
        eprintln!("Cannot apply: {error}");
        return ExitCode::FAILURE;
    }
    match apply::execute(source, &plan) {
        Ok(count) => {
            println!("Renamed {count} media files.");
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("Rename failed: {}", failure.trigger);
            if failure.recovery.is_empty() {
                eprintln!("Any files moved by this operation were restored.");
            }
            for entry in failure.recovery {
                eprintln!(
                    "Manual recovery needed for {:?}. Last known location (unverified): {:?}. {}",
                    entry.original, entry.last_known, entry.error
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn confirm_apply() -> io::Result<bool> {
    write_prompt("Type yes and press Enter to apply these renames: ")?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.as_str(), "yes\n" | "yes\r\n"))
}

fn write_prompt(prompt: &str) -> io::Result<()> {
    let mut output = io::stdout().lock();
    write!(output, "{prompt}")?;
    output.flush()
}

fn prompt_folder() -> io::Result<Option<PathBuf>> {
    write_prompt("Folder path: ")?;

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
