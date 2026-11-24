//! Prepare and revalidate the exact plan reviewed before execution.

use super::{
    Fingerprint, MediaSource, PreviewRow, RenameSource, SourceObservation,
    analyze_with_observations,
};
use crate::domain::AnalysisReport;
use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SourceEvidence {
    pub(super) observation: SourceObservation,
    pub(super) fingerprint: Fingerprint,
    pub(super) timestamp: Option<SystemTime>,
}

pub(crate) struct PreparedApply {
    pub(crate) root: PathBuf,
    pub(crate) report: AnalysisReport,
    pub(crate) rows: Vec<PreviewRow>,
    sources: HashMap<PathBuf, SourceEvidence>,
}

pub(crate) fn prepare_apply(
    source: &(impl MediaSource + RenameSource),
    selected_root: &Path,
) -> io::Result<PreparedApply> {
    let root = source.canonical_root(selected_root)?;
    let (analysis, sources) = analyze_with_observations(source, &root, true, Some(source))?;
    Ok(PreparedApply {
        root,
        report: analysis.report,
        rows: analysis.preview.unwrap_or_default(),
        sources,
    })
}

pub(crate) fn revalidate(
    source: &(impl MediaSource + RenameSource),
    selected_root: &Path,
    approved: &PreparedApply,
) -> io::Result<()> {
    if approved.rows.iter().any(|row| row.blocked) {
        return Err(io::Error::new(
            ErrorKind::AlreadyExists,
            "Cannot apply a plan with occupied destinations",
        ));
    }
    let current = prepare_apply(source, selected_root)?;
    if current.root != approved.root
        || current.rows != approved.rows
        || current.sources != approved.sources
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "Folder or media changed after the preview. Review a new plan before applying.",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct ApplyFailure {
    pub(crate) trigger: io::Error,
    pub(crate) recovery: Vec<RecoveryFailure>,
}

#[derive(Debug)]
pub(crate) struct RecoveryFailure {
    pub(crate) original: PathBuf,
    pub(crate) last_known: PathBuf,
    pub(crate) error: io::Error,
}

#[derive(Eq, PartialEq)]
enum Location {
    Original,
    Staged,
    Final,
}

struct PendingMove<'a> {
    original: &'a Path,
    staged: PathBuf,
    destination: &'a Path,
    observation: &'a SourceObservation,
    location: Location,
}

pub(crate) fn execute(
    source: &impl RenameSource,
    approved: &PreparedApply,
) -> Result<usize, ApplyFailure> {
    if approved.rows.iter().any(|row| row.blocked) {
        return Err(ApplyFailure {
            trigger: io::Error::new(
                ErrorKind::AlreadyExists,
                "Cannot apply a plan with occupied destinations",
            ),
            recovery: Vec::new(),
        });
    }

    let mut moves: Vec<_> = approved
        .rows
        .iter()
        .filter(|row| row.name.source != row.name.destination)
        .enumerate()
        .map(|(index, row)| PendingMove {
            original: &row.name.source,
            staged: stage_path(&row.name.source, index, 0),
            destination: &row.name.destination,
            observation: &approved.sources[&row.name.source].observation,
            location: Location::Original,
        })
        .collect();

    match perform_moves(source, &approved.root, &mut moves) {
        Ok(()) => Ok(moves.len()),
        Err(trigger) => Err(ApplyFailure {
            trigger,
            recovery: restore(source, &approved.root, &mut moves),
        }),
    }
}

fn stage_path(original: &Path, index: usize, attempt: u8) -> PathBuf {
    original.with_file_name(format!(
        ".media-organizer-{}-{index}-{attempt}.stage",
        std::process::id()
    ))
}

fn perform_moves(
    source: &impl RenameSource,
    root: &Path,
    moves: &mut [PendingMove<'_>],
) -> io::Result<()> {
    for (index, movement) in moves.iter_mut().enumerate() {
        for attempt in 0..16 {
            movement.staged = stage_path(movement.original, index, attempt);
            match source.move_exclusive(
                root,
                movement.original,
                &movement.staged,
                movement.observation,
            ) {
                Ok(()) => {
                    movement.location = Location::Staged;
                    break;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists && attempt < 15 => {}
                Err(error) => return Err(error),
            }
        }
    }

    for movement in moves {
        source.move_exclusive(
            root,
            &movement.staged,
            movement.destination,
            movement.observation,
        )?;
        movement.location = Location::Final;
    }
    Ok(())
}

fn restore(
    source: &impl RenameSource,
    root: &Path,
    moves: &mut [PendingMove<'_>],
) -> Vec<RecoveryFailure> {
    // Vacate final names before restoring, since those names may be other originals.
    for movement in moves
        .iter_mut()
        .rev()
        .filter(|movement| movement.location == Location::Final)
    {
        if source
            .move_exclusive(
                root,
                movement.destination,
                &movement.staged,
                movement.observation,
            )
            .is_ok()
        {
            movement.location = Location::Staged;
        }
    }

    let mut failures = Vec::new();
    for movement in moves.iter_mut().rev() {
        let last_known = match movement.location {
            Location::Original => continue,
            Location::Staged => &movement.staged,
            Location::Final => movement.destination,
        };
        match source.move_exclusive(root, last_known, movement.original, movement.observation) {
            Ok(()) => movement.location = Location::Original,
            Err(error) => failures.push(RecoveryFailure {
                original: movement.original.to_path_buf(),
                last_known: last_known.to_path_buf(),
                error,
            }),
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{LocalMediaSource, tests::Scratch};
    use std::fs;

    #[test]
    fn revalidates_unchanged_plan_and_rejects_content_changes() {
        let scratch = Scratch::new();
        let image = scratch.0.join("holiday.JPG");
        fs::write(&image, b"original").unwrap();
        let plan = prepare_apply(&LocalMediaSource, &scratch.0).unwrap();
        assert_eq!(plan.rows[0].name.destination.file_name().unwrap(), "1.JPG");
        revalidate(&LocalMediaSource, &scratch.0, &plan).unwrap();

        fs::write(&image, b"replaced").unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());
        assert_eq!(fs::read(&image).unwrap(), b"replaced");
        assert!(!scratch.0.join("1.JPG").exists());
    }

    #[test]
    fn rejects_a_late_obstacle_and_a_removed_source() {
        let scratch = Scratch::new();
        let image = scratch.0.join("holiday.jpg");
        let destination = scratch.0.join("1.jpg");
        fs::write(&image, b"original").unwrap();
        let plan = prepare_apply(&LocalMediaSource, &scratch.0).unwrap();
        fs::create_dir(&destination).unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());

        let blocked = prepare_apply(&LocalMediaSource, &scratch.0).unwrap();
        assert!(blocked.rows[0].blocked);
        assert!(revalidate(&LocalMediaSource, &scratch.0, &blocked).is_err());
        fs::remove_dir(&destination).unwrap();
        fs::remove_file(&image).unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());
    }

    #[test]
    fn rejects_added_media_that_would_change_the_plan() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("holiday.jpg"), b"original").unwrap();
        let plan = prepare_apply(&LocalMediaSource, &scratch.0).unwrap();
        fs::write(scratch.0.join("new.jpg"), b"newly added").unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());
    }
}

#[cfg(test)]
mod execution_tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeMoves {
        files: RefCell<HashMap<PathBuf, u8>>,
        failures: RefCell<Vec<(Option<PathBuf>, PathBuf)>>,
    }

    impl RenameSource for FakeMoves {
        fn canonical_root(&self, _: &Path) -> io::Result<PathBuf> {
            unreachable!()
        }

        fn observe_source(&self, _: &Path, _: &Path) -> io::Result<SourceObservation> {
            unreachable!()
        }

        fn move_exclusive(
            &self,
            _: &Path,
            source: &Path,
            destination: &Path,
            _: &SourceObservation,
        ) -> io::Result<()> {
            let mut failures = self.failures.borrow_mut();
            if let Some(index) = failures.iter().position(|(from, to)| {
                from.as_deref().is_none_or(|from| from == source) && to == destination
            }) {
                failures.remove(index);
                return Err(io::Error::other("injected failure"));
            }
            let mut files = self.files.borrow_mut();
            if files.contains_key(destination) {
                return Err(ErrorKind::AlreadyExists.into());
            }
            let bytes = files.remove(source).ok_or(ErrorKind::NotFound)?;
            files.insert(destination.to_path_buf(), bytes);
            Ok(())
        }
    }

    fn sample(failures: Vec<(Option<PathBuf>, PathBuf)>) -> (FakeMoves, PreparedApply) {
        let root = PathBuf::from("root");
        let first = root.join("1.jpg");
        let second = root.join("2.jpg");
        let files = [(first.clone(), 1), (second.clone(), 2)].into();
        let sources = [first.clone(), second.clone()]
            .into_iter()
            .map(|path| {
                (
                    path,
                    SourceEvidence {
                        observation: SourceObservation {
                            len: 1,
                            modified: None,
                            created: None,
                            #[cfg(unix)]
                            device_inode: (1, 1),
                        },
                        fingerprint: Fingerprint {
                            byte_count: 1,
                            hash: 1,
                        },
                        timestamp: None,
                    },
                )
            })
            .collect();
        let rows = vec![
            PreviewRow {
                name: crate::domain::preview::ProposedName {
                    source: first.clone(),
                    destination: second.clone(),
                },
                blocked: false,
            },
            PreviewRow {
                name: crate::domain::preview::ProposedName {
                    source: second,
                    destination: first,
                },
                blocked: false,
            },
        ];
        (
            FakeMoves {
                files: RefCell::new(files),
                failures: RefCell::new(failures),
            },
            PreparedApply {
                root,
                report: AnalysisReport::default(),
                rows,
                sources,
            },
        )
    }

    #[test]
    fn stages_all_sources_before_applying_a_swap() {
        let (adapter, plan) = sample(Vec::new());
        assert_eq!(execute(&adapter, &plan).unwrap(), 2);
        assert_eq!(
            *adapter.files.borrow(),
            [("root/1.jpg".into(), 2), ("root/2.jpg".into(), 1)].into()
        );
    }

    #[test]
    fn restores_already_finalized_and_staged_sources() {
        let (adapter, plan) = sample(vec![(None, "root/1.jpg".into())]);
        let error = execute(&adapter, &plan).unwrap_err();
        assert!(error.recovery.is_empty());
        assert_eq!(
            *adapter.files.borrow(),
            [("root/1.jpg".into(), 1), ("root/2.jpg".into(), 2)].into()
        );
    }

    #[test]
    fn staging_failure_restores_other_sources_and_keeps_occupied_stage_names() {
        let second = Path::new("root/2.jpg");
        let occupied = stage_path(second, 1, 0);
        let failed = stage_path(second, 1, 1);
        let (adapter, plan) = sample(vec![(Some(second.to_path_buf()), failed)]);
        adapter.files.borrow_mut().insert(occupied.clone(), 9);
        let error = execute(&adapter, &plan).unwrap_err();

        assert!(error.recovery.is_empty());
        assert_eq!(
            *adapter.files.borrow(),
            [
                ("root/1.jpg".into(), 1),
                ("root/2.jpg".into(), 2),
                (occupied, 9)
            ]
            .into()
        );
    }

    #[test]
    fn reports_last_known_location_when_a_restore_fails() {
        let (adapter, plan) = sample(vec![
            (None, "root/1.jpg".into()),
            (None, "root/1.jpg".into()),
        ]);
        let error = execute(&adapter, &plan).unwrap_err();
        assert_eq!(error.recovery.len(), 1);
        let unresolved = &error.recovery[0];
        assert_eq!(unresolved.original, Path::new("root/1.jpg"));
        assert_ne!(unresolved.last_known, unresolved.original);
        assert_eq!(adapter.files.borrow()[&unresolved.last_known], 1);
        assert_eq!(adapter.files.borrow()[Path::new("root/2.jpg")], 2);
    }
}
