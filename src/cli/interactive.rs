use super::{
    apply, apply_prepared, confirm_apply, has_changes, print_plan_preview, print_report, terminal,
};
use crate::application::{self, DuplicateRemoval, MediaSource, OrganizationOptions, RenameSource};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

trait GuidedInput {
    fn folder(&mut self) -> io::Result<Option<PathBuf>>;
    fn choose(&mut self, label: &str, choices: &[&str]) -> io::Result<Option<usize>>;
    fn confirm(&mut self, changes_include_more_than_renaming: bool) -> io::Result<bool>;
}

struct Terminal;

impl GuidedInput for Terminal {
    fn folder(&mut self) -> io::Result<Option<PathBuf>> {
        terminal::folder()
    }

    fn choose(&mut self, label: &str, choices: &[&str]) -> io::Result<Option<usize>> {
        terminal::choose(label, choices)
    }

    fn confirm(&mut self, changes_include_more_than_renaming: bool) -> io::Result<bool> {
        confirm_apply(changes_include_more_than_renaming)
    }
}

pub(super) fn run(source: &(impl MediaSource + RenameSource + DuplicateRemoval)) -> ExitCode {
    match session(source, &mut Terminal) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Terminal error: {}", terminal::escaped(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

fn session(
    source: &(impl MediaSource + RenameSource + DuplicateRemoval),
    input: &mut impl GuidedInput,
) -> io::Result<ExitCode> {
    loop {
        let Some(folder) = input.folder()? else {
            return Ok(ExitCode::SUCCESS);
        };
        let analysis = match application::analyze_folder(
            source,
            &folder,
            false,
            OrganizationOptions::default(),
        ) {
            Ok(analysis) => analysis,
            Err(error) => {
                eprintln!("Analysis failed: {}", terminal::escaped(&error.to_string()));
                continue;
            }
        };
        print_report(&analysis.report);
        let Some(remove_duplicates) = input.choose("Remove duplicate copies?", &["No", "Yes"])?
        else {
            return Ok(ExitCode::SUCCESS);
        };
        let Some(flatten) = input.choose("Move media into the selected folder?", &["No", "Yes"])?
        else {
            return Ok(ExitCode::SUCCESS);
        };
        let options = OrganizationOptions {
            remove_duplicates: remove_duplicates == 1,
            flatten: flatten == 1,
        };
        let plan = match apply::prepare_apply(source, &folder, options) {
            Ok(plan) => plan,
            Err(error) => {
                eprintln!("Analysis failed: {}", terminal::escaped(&error.to_string()));
                continue;
            }
        };
        print_plan_preview(&plan, options);
        let actions = if plan.rows.iter().any(|row| row.blocked) {
            eprintln!("Cannot apply: the preview has occupied destinations.");
            &["Exit", "Restart"][..]
        } else if !has_changes(&plan) {
            println!("No files need renaming.");
            &["Exit", "Restart"][..]
        } else {
            &["Exit", "Restart", "Apply"][..]
        };
        match input.choose("Next action", actions)? {
            Some(1) => {}
            Some(2) => {
                return Ok(apply_prepared(source, &folder, &plan, options, |changes| {
                    input.confirm(changes)
                }));
            }
            _ => return Ok(ExitCode::SUCCESS),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{LocalMediaSource, tests::Scratch};
    use std::{collections::VecDeque, fs};

    struct Script {
        folder: PathBuf,
        selections: VecDeque<usize>,
        confirm: bool,
        confirmations: usize,
    }

    impl GuidedInput for Script {
        fn folder(&mut self) -> io::Result<Option<PathBuf>> {
            Ok(Some(self.folder.clone()))
        }

        fn choose(&mut self, _: &str, choices: &[&str]) -> io::Result<Option<usize>> {
            let selected = self.selections.pop_front().unwrap();
            assert!(selected < choices.len());
            Ok(Some(selected))
        }

        fn confirm(&mut self, _: bool) -> io::Result<bool> {
            self.confirmations += 1;
            Ok(self.confirm)
        }
    }

    #[test]
    fn restarting_resets_choices_and_remains_read_only() {
        let root = Scratch::new();
        std::fs::write(root.0.join("a.JPG"), b"same").unwrap();
        std::fs::write(root.0.join("b.JPG"), b"same").unwrap();
        let mut script = Script {
            folder: root.0.clone(),
            selections: [1, 1, 1, 0, 0, 0].into(),
            confirm: true,
            confirmations: 0,
        };
        assert_eq!(
            session(&LocalMediaSource, &mut script).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(script.confirmations, 0);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
        assert!(root.0.join("a.JPG").is_file());
        assert!(root.0.join("b.JPG").is_file());
    }

    #[test]
    fn selected_apply_still_requires_confirmation() {
        let root = Scratch::new();
        fs::write(root.0.join("holiday.JPG"), b"original").unwrap();
        let mut script = Script {
            folder: root.0.clone(),
            selections: [0, 0, 2].into(),
            confirm: false,
            confirmations: 0,
        };
        assert_eq!(
            session(&LocalMediaSource, &mut script).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(script.confirmations, 1);
        assert_eq!(fs::read(root.0.join("holiday.JPG")).unwrap(), b"original");

        script.selections = [0, 0, 2].into();
        script.confirm = true;
        assert_eq!(
            session(&LocalMediaSource, &mut script).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(script.confirmations, 2);
        assert!(!root.0.join("holiday.JPG").exists());
        assert_eq!(fs::read(root.0.join("1.JPG")).unwrap(), b"original");
    }
}
