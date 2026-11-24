mod interactive;

mod terminal;

use crate::application::{
    self, DuplicateCopy, DuplicateRemoval, MediaSource, OrganizationOptions, PreviewRow,
    RenameSource, apply,
};
use crate::domain::{AnalysisReport, MediaCounts};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn run(
    mut arguments: impl Iterator<Item = OsString>,
    source: &(impl MediaSource + RenameSource + DuplicateRemoval),
) -> ExitCode {
    let mut first = arguments.next();
    if first.is_none() && io::stdin().is_terminal() && io::stdout().is_terminal() {
        return interactive::run(source);
    }
    let mut apply = false;
    let mut preview = false;
    let mut options = OrganizationOptions::default();
    loop {
        match first.as_deref().and_then(|argument| argument.to_str()) {
            Some("--apply") if !apply && !preview => apply = true,
            Some("--preview") if !apply && !preview => preview = true,
            Some("--remove-duplicates") if !options.remove_duplicates => {
                options.remove_duplicates = true
            }
            Some("--flatten") if !options.flatten => options.flatten = true,
            _ => break,
        }
        first = arguments.next();
    }
    if (options.remove_duplicates || options.flatten) && !(apply || preview) {
        return usage_error();
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
                println!("       Add --flatten or --remove-duplicates to --preview or --apply.");
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
        return apply_folder(source, &folder, options);
    }

    match application::analyze_folder(source, &folder, preview, options) {
        Ok(analysis) => {
            print_report(&analysis.report);
            if let Some(rows) = analysis.preview {
                print_preview(&folder, &rows);
                print_duplicates(
                    &folder,
                    &rows,
                    &analysis.duplicates,
                    options.remove_duplicates,
                );
                if options.flatten {
                    print_flatten_notice();
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Analysis failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn apply_folder(
    source: &(impl MediaSource + RenameSource + DuplicateRemoval),
    folder: &Path,
    options: OrganizationOptions,
) -> ExitCode {
    let plan = match apply::prepare_apply(source, folder, options) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("Analysis failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    print_report(&plan.report);
    print_plan_preview(&plan, options);
    apply_prepared(source, folder, &plan, options, confirm_apply)
}

fn print_plan_preview(plan: &apply::PreparedApply, options: OrganizationOptions) {
    print_preview(&plan.root, &plan.rows);
    print_duplicates(
        &plan.root,
        &plan.rows,
        &plan.duplicates,
        options.remove_duplicates,
    );
    if options.flatten {
        print_flatten_notice();
    }
}

fn has_changes(plan: &apply::PreparedApply) -> bool {
    !plan.duplicates.is_empty()
        || plan
            .rows
            .iter()
            .any(|row| row.name.source != row.name.destination)
}

fn apply_prepared(
    source: &(impl MediaSource + RenameSource + DuplicateRemoval),
    folder: &Path,
    plan: &apply::PreparedApply,
    options: OrganizationOptions,
    confirm: impl FnOnce(bool) -> io::Result<bool>,
) -> ExitCode {
    if plan.rows.iter().any(|row| row.blocked) {
        eprintln!("Cannot apply: the preview has occupied destinations.");
        return ExitCode::FAILURE;
    }
    if !has_changes(plan) {
        println!("No files need renaming.");
        return ExitCode::SUCCESS;
    }

    if !plan.duplicates.is_empty() {
        println!("This will permanently delete the listed duplicate copies.");
    }
    let changes_include_more_than_renaming = options.flatten || !plan.duplicates.is_empty();
    match confirm(changes_include_more_than_renaming) {
        Ok(false) => {
            if !changes_include_more_than_renaming {
                println!("Cancelled. No files were renamed.");
            } else {
                println!("Cancelled. No files were changed.");
            }
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("Could not read confirmation: {error}");
            return ExitCode::FAILURE;
        }
        Ok(true) => {}
    }
    if let Err(error) = apply::revalidate(source, folder, plan) {
        eprintln!("Cannot apply: {error}");
        return ExitCode::FAILURE;
    }
    match apply::execute(source, plan) {
        Ok(summary) => {
            println!("Renamed {} media files.", summary.renamed);
            if options.remove_duplicates {
                println!("Removed {} duplicate files.", summary.removed);
            }
            if options.flatten {
                println!("Removed {} empty subfolders.", summary.directories_removed);
            }
            if !summary.directory_errors.is_empty() {
                for error in summary.directory_errors {
                    eprintln!("Subfolder cleanup failed after applying file changes: {error}");
                }
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(apply::CleanupError::Rename(failure)) => {
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
        Err(apply::CleanupError::Removal {
            trigger,
            removed,
            remaining,
        }) => {
            eprintln!("Duplicate cleanup stopped after renaming: {trigger}");
            eprintln!("Removed {} duplicate files:", removed.len());
            for original in removed {
                eprintln!("Removed copy from original path {original:?}");
            }
            eprintln!("Remaining duplicate copies: {}", remaining.len());
            for (original, staged) in remaining {
                eprintln!(
                    "Original copy {original:?}. Last known staged location (unverified): {staged:?}"
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn print_flatten_notice() {
    println!("Only affected subfolders that become empty will be removed.");
}

fn confirm_apply(changes_include_more_than_renaming: bool) -> io::Result<bool> {
    let prompt = if changes_include_more_than_renaming {
        "Type yes and press Enter to apply these changes: "
    } else {
        "Type yes and press Enter to apply these renames: "
    };
    write_prompt(prompt)?;
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

fn print_duplicates(root: &Path, rows: &[PreviewRow], duplicates: &[DuplicateCopy], enabled: bool) {
    if !enabled {
        return;
    }
    println!("Duplicate removal preview:");
    if duplicates.is_empty() {
        println!("No duplicate copies to remove.");
        return;
    }
    let destinations: HashMap<_, _> = rows
        .iter()
        .map(|row| (row.name.source.as_path(), row.name.destination.as_path()))
        .collect();
    for entry in duplicates {
        let copy = relative_to(root, &entry.copy);
        let keeper = relative_to(root, &entry.keeper);
        let final_path = relative_to(root, destinations[entry.keeper.as_path()]);
        if keeper == final_path {
            println!("Remove {copy:?} and keep {keeper:?}");
        } else {
            println!("Remove {copy:?} and keep {keeper:?} as {final_path:?}");
        }
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
