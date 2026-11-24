//! Prepare and revalidate the exact plan reviewed before execution.

use super::{
    DuplicateCopy, DuplicateRemoval, Fingerprint, MediaSource, PreviewRow, RenameSource,
    SourceObservation, StagedCopy, analyze_with_observations,
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
    pub(crate) duplicates: Vec<DuplicateCopy>,
    remove_duplicates: bool,
    sources: HashMap<PathBuf, SourceEvidence>,
}

pub(crate) fn prepare_apply(
    source: &(impl MediaSource + RenameSource),
    selected_root: &Path,
    remove_duplicates: bool,
) -> io::Result<PreparedApply> {
    let root = source.canonical_root(selected_root)?;
    let (analysis, sources) =
        analyze_with_observations(source, &root, true, remove_duplicates, Some(source))?;
    Ok(PreparedApply {
        root,
        report: analysis.report,
        rows: analysis.preview.unwrap_or_default(),
        duplicates: analysis.duplicates,
        remove_duplicates,
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
    let current = prepare_apply(source, selected_root, approved.remove_duplicates)?;
    if current.root != approved.root
        || current.rows != approved.rows
        || current.duplicates != approved.duplicates
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
    destination: Option<&'a Path>,
    observation: &'a SourceObservation,
    location: Location,
}

fn finish_renames<'a>(
    source: &impl RenameSource,
    approved: &'a PreparedApply,
) -> Result<Vec<PendingMove<'a>>, ApplyFailure> {
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
        .map(|row| {
            (
                row.name.source.as_path(),
                Some(row.name.destination.as_path()),
            )
        })
        .chain(
            approved
                .duplicates
                .iter()
                .map(|copy| (copy.copy.as_path(), None)),
        )
        .enumerate()
        .map(|(index, (original, destination))| PendingMove {
            original,
            staged: stage_path(original, index, 0),
            destination,
            observation: &approved.sources[original].observation,
            location: Location::Original,
        })
        .collect();

    match perform_moves(source, &approved.root, &mut moves) {
        Ok(()) => Ok(moves),
        Err(trigger) => Err(ApplyFailure {
            trigger,
            recovery: restore(source, &approved.root, &mut moves),
        }),
    }
}

#[derive(Debug)]
pub(crate) struct ApplySummary {
    pub(crate) renamed: usize,
    pub(crate) removed: usize,
}

#[derive(Debug)]
pub(crate) enum CleanupError {
    Rename(ApplyFailure),
    Removal {
        trigger: io::Error,
        removed: Vec<PathBuf>,
        remaining: Vec<(PathBuf, PathBuf)>,
    },
}

pub(crate) fn execute(
    source: &(impl RenameSource + DuplicateRemoval),
    approved: &PreparedApply,
) -> Result<ApplySummary, CleanupError> {
    let moves = finish_renames(source, approved).map_err(CleanupError::Rename)?;
    let renamed = moves.len() - approved.duplicates.len();
    let destinations: HashMap<_, _> = approved
        .rows
        .iter()
        .map(|row| (row.name.source.as_path(), row.name.destination.as_path()))
        .collect();
    let mut staged = moves[renamed..].iter();
    let mut removed = Vec::new();
    for copy in &approved.duplicates {
        let movement = staged.next().expect("one stage per duplicate");
        let request = StagedCopy {
            path: &movement.staged,
            observation: movement.observation,
            keeper: destinations[copy.keeper.as_path()],
            keeper_observation: &approved.sources[&copy.keeper].observation,
        };
        if let Err(trigger) = source.remove_staged_copy(&approved.root, request) {
            let remaining = std::iter::once(movement)
                .chain(staged)
                .map(|entry| (entry.original.to_path_buf(), entry.staged.clone()))
                .collect();
            return Err(CleanupError::Removal {
                trigger,
                removed,
                remaining,
            });
        }
        removed.push(movement.original.to_path_buf());
    }
    Ok(ApplySummary {
        renamed,
        removed: removed.len(),
    })
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
        let Some(destination) = movement.destination else {
            continue;
        };
        source.move_exclusive(root, &movement.staged, destination, movement.observation)?;
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
                movement.destination.expect("final move has a destination"),
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
            Location::Final => movement.destination.expect("final move has a destination"),
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
    fn cleanup_uses_the_keeper_final_path() {
        let scratch = Scratch::new();
        let incoming = scratch.0.join("a");
        fs::create_dir(&incoming).unwrap();
        fs::write(incoming.join("a.JPG"), b"same").unwrap();
        fs::write(incoming.join("z.JPG"), b"same").unwrap();
        fs::write(scratch.0.join("other.JPG"), b"same").unwrap();
        fs::write(scratch.0.join("1.JPG"), b"diff").unwrap();

        let plan = prepare_apply(&LocalMediaSource, &scratch.0, true).unwrap();
        let result = execute(&LocalMediaSource, &plan).unwrap();
        assert_eq!(result.renamed, 1);
        assert_eq!(result.removed, 2);
        assert_eq!(fs::read(incoming.join("1.JPG")).unwrap(), b"same");
        assert_eq!(fs::read(scratch.0.join("1.JPG")).unwrap(), b"diff");
        assert_eq!(fs::read_dir(&incoming).unwrap().count(), 1);
        assert!(!scratch.0.join("other.JPG").exists());
    }

    #[test]
    fn cleanup_can_vacate_a_survivor_target_and_preserves_the_keeper_hardlink() {
        let scratch = Scratch::new();
        let earlier = scratch.0.join("0");
        fs::create_dir(&earlier).unwrap();
        fs::write(earlier.join("a.JPG"), b"same").unwrap();
        fs::hard_link(earlier.join("a.JPG"), scratch.0.join("1.JPG")).unwrap();
        fs::write(scratch.0.join("survivor.JPG"), b"diff").unwrap();

        let plan = prepare_apply(&LocalMediaSource, &scratch.0, true).unwrap();
        assert_eq!(plan.duplicates.len(), 1);
        let result = execute(&LocalMediaSource, &plan).unwrap();
        assert_eq!(result.renamed, 2);
        assert_eq!(result.removed, 1);
        assert_eq!(fs::read(earlier.join("1.JPG")).unwrap(), b"same");
        assert_eq!(fs::read(scratch.0.join("1.JPG")).unwrap(), b"diff");
    }

    #[test]
    fn duplicate_plan_keeps_first_path_and_uses_only_survivors_for_names() {
        let scratch = Scratch::new();
        let a = scratch.0.join("a");
        let z = scratch.0.join("z");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&z).unwrap();
        fs::write(a.join("keeper.jpg"), b"same").unwrap();
        fs::write(z.join("1.jpg"), b"same").unwrap();
        fs::write(z.join("other.jpg"), b"diff").unwrap();

        let plan = prepare_apply(&LocalMediaSource, &scratch.0, true).unwrap();
        let root = &plan.root;
        assert_eq!(plan.report.total().files, 3);
        assert_eq!(plan.report.total().duplicates, 1);
        assert_eq!(plan.duplicates[0].copy, root.join("z/1.jpg"));
        assert_eq!(plan.duplicates[0].keeper, root.join("a/keeper.jpg"));
        assert_eq!(plan.rows.len(), 2);
        assert_eq!(plan.rows[1].name.destination, root.join("z/1.jpg"));
        assert!(!plan.rows[1].blocked);
        revalidate(&LocalMediaSource, &scratch.0, &plan).unwrap();
        fs::write(z.join("1.jpg"), b"late").unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());
    }

    #[test]
    fn revalidates_unchanged_plan_and_rejects_content_changes() {
        let scratch = Scratch::new();
        let image = scratch.0.join("holiday.JPG");
        fs::write(&image, b"original").unwrap();
        let plan = prepare_apply(&LocalMediaSource, &scratch.0, false).unwrap();
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
        let plan = prepare_apply(&LocalMediaSource, &scratch.0, false).unwrap();
        fs::create_dir(&destination).unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());

        let blocked = prepare_apply(&LocalMediaSource, &scratch.0, false).unwrap();
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
        let plan = prepare_apply(&LocalMediaSource, &scratch.0, false).unwrap();
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
        cleanup_failure: Option<PathBuf>,
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

    impl DuplicateRemoval for FakeMoves {
        fn remove_staged_copy(&self, _: &Path, copy: StagedCopy<'_>) -> io::Result<()> {
            let mut files = self.files.borrow_mut();
            assert_eq!(files[copy.keeper], files[copy.path]);
            if self.cleanup_failure.as_deref() == Some(copy.path) {
                return Err(io::Error::other("injected cleanup failure"));
            }
            files.remove(copy.path).unwrap();
            Ok(())
        }
    }

    fn add_copy(adapter: &FakeMoves, plan: &mut PreparedApply, copy: &str) {
        let copy = plan.root.join(copy);
        let keeper = plan.root.join("1.jpg");
        let evidence = &plan.sources[&keeper];
        plan.sources.insert(
            copy.clone(),
            SourceEvidence {
                observation: evidence.observation.clone(),
                fingerprint: evidence.fingerprint,
                timestamp: evidence.timestamp,
            },
        );
        plan.duplicates.push(DuplicateCopy {
            copy: copy.clone(),
            keeper,
        });
        adapter.files.borrow_mut().insert(copy, 1);
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
                cleanup_failure: None,
            },
            PreparedApply {
                root,
                report: AnalysisReport::default(),
                rows,
                duplicates: Vec::new(),
                remove_duplicates: false,
                sources,
            },
        )
    }

    #[test]
    fn finalization_failure_restores_staged_copies_without_unlinking() {
        let (adapter, mut plan) = sample(vec![(None, "root/1.jpg".into())]);
        add_copy(&adapter, &mut plan, "copy.jpg");
        let before = adapter.files.borrow().clone();

        let CleanupError::Rename(error) = execute(&adapter, &plan).unwrap_err() else {
            panic!("expected rename failure");
        };
        assert!(error.recovery.is_empty());
        assert_eq!(*adapter.files.borrow(), before);
    }

    #[test]
    fn partial_cleanup_reports_removed_copies_and_preserves_every_remaining_stage() {
        let (mut adapter, mut plan) = sample(Vec::new());
        for name in ["copy1.jpg", "copy2.jpg", "copy3.jpg"] {
            add_copy(&adapter, &mut plan, name);
        }
        adapter.cleanup_failure = Some(stage_path(&plan.root.join("copy2.jpg"), 3, 0));

        let CleanupError::Removal {
            trigger,
            removed,
            remaining,
        } = execute(&adapter, &plan).unwrap_err()
        else {
            panic!("expected cleanup failure");
        };
        assert_eq!(trigger.to_string(), "injected cleanup failure");
        assert_eq!(removed, [plan.root.join("copy1.jpg")]);
        let files = adapter.files.borrow();
        assert_eq!(files[&plan.root.join("2.jpg")], 1);
        assert_eq!(remaining.len(), 2);
        for (index, (original, staged)) in remaining.iter().enumerate() {
            assert_eq!(*original, plan.root.join(format!("copy{}.jpg", index + 2)));
            assert!(!files.contains_key(original));
            assert_eq!(files[staged], 1);
        }
    }

    #[test]
    fn stages_all_sources_before_applying_a_swap() {
        let (adapter, plan) = sample(Vec::new());
        assert_eq!(execute(&adapter, &plan).unwrap().renamed, 2);
        assert_eq!(
            *adapter.files.borrow(),
            [("root/1.jpg".into(), 2), ("root/2.jpg".into(), 1)].into()
        );
    }

    #[test]
    fn restores_already_finalized_and_staged_sources() {
        let (adapter, plan) = sample(vec![(None, "root/1.jpg".into())]);
        let CleanupError::Rename(error) = execute(&adapter, &plan).unwrap_err() else {
            panic!("expected rename failure");
        };
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
        let CleanupError::Rename(error) = execute(&adapter, &plan).unwrap_err() else {
            panic!("expected rename failure");
        };

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
        let CleanupError::Rename(error) = execute(&adapter, &plan).unwrap_err() else {
            panic!("expected rename failure");
        };
        assert_eq!(error.recovery.len(), 1);
        let unresolved = &error.recovery[0];
        assert_eq!(unresolved.original, Path::new("root/1.jpg"));
        assert_ne!(unresolved.last_known, unresolved.original);
        assert_eq!(adapter.files.borrow()[&unresolved.last_known], 1);
        assert_eq!(adapter.files.borrow()[Path::new("root/2.jpg")], 2);
    }
}
