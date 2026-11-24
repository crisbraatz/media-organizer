//! Prepare and revalidate the exact plan reviewed before execution.

use super::{
    DirectoryObservation, DuplicateCopy, DuplicateRemoval, Fingerprint, MediaSource,
    OrganizationOptions, PreviewRow, RenameSource, SourceObservation, StagedCopy,
    analyze_with_observations,
};
use crate::domain::AnalysisReport;
use std::collections::{HashMap, HashSet};
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
    options: OrganizationOptions,
    directories: Vec<(PathBuf, DirectoryObservation)>,
    sources: HashMap<PathBuf, SourceEvidence>,
}

pub(crate) fn prepare_apply(
    source: &(impl MediaSource + RenameSource),
    selected_root: &Path,
    options: OrganizationOptions,
) -> io::Result<PreparedApply> {
    let root = source.canonical_root(selected_root)?;
    let (analysis, sources) =
        analyze_with_observations(source, &root, true, options, Some(source))?;
    let rows = analysis.preview.unwrap_or_default();
    let directories = if options.flatten {
        let mut candidates: Vec<_> = rows
            .iter()
            .filter(|row| row.name.source.parent() != row.name.destination.parent())
            .map(|row| row.name.source.as_path())
            .chain(analysis.duplicates.iter().map(|copy| copy.copy.as_path()))
            .flat_map(|path| {
                path.ancestors()
                    .skip(1)
                    .take_while(|parent| *parent != root && parent.starts_with(&root))
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        candidates.sort_unstable_by(|left, right| {
            right
                .components()
                .count()
                .cmp(&left.components().count())
                .then_with(|| left.cmp(right))
        });
        candidates
            .into_iter()
            .map(|path| Ok((path.to_path_buf(), source.observe_directory(&root, path)?)))
            .collect::<io::Result<_>>()?
    } else {
        Vec::new()
    };
    Ok(PreparedApply {
        root,
        report: analysis.report,
        rows,
        duplicates: analysis.duplicates,
        options,
        directories,
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
    let current = prepare_apply(source, selected_root, approved.options)?;
    if current.root != approved.root
        || current.rows != approved.rows
        || current.duplicates != approved.duplicates
        || current.sources != approved.sources
        || current.directories != approved.directories
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    Staging,
    Renaming,
    RemovingDuplicates,
    CheckingDirectories,
    Restoring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Progress {
    pub(crate) phase: Phase,
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

impl Phase {
    fn snapshot(self, completed: usize, total: usize) -> Progress {
        Progress {
            phase: self,
            completed,
            total,
        }
    }

    fn start(self, total: usize, report: &mut impl FnMut(Progress)) {
        if total != 0 {
            report(self.snapshot(0, total));
        }
    }
}

fn finish_renames<'a>(
    source: &impl RenameSource,
    approved: &'a PreparedApply,
    report: &mut impl FnMut(Progress),
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

    match perform_moves(source, &approved.root, &mut moves, report) {
        Ok(()) => Ok(moves),
        Err(trigger) => Err(ApplyFailure {
            trigger,
            recovery: restore(source, &approved.root, &mut moves, report),
        }),
    }
}

#[derive(Debug)]
pub(crate) struct ApplySummary {
    pub(crate) renamed: usize,
    pub(crate) removed: usize,
    pub(crate) directories_removed: usize,
    pub(crate) directory_errors: Vec<io::Error>,
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
    mut report: impl FnMut(Progress),
) -> Result<ApplySummary, CleanupError> {
    let moves = finish_renames(source, approved, &mut report).map_err(CleanupError::Rename)?;
    let renamed = moves.len() - approved.duplicates.len();
    let destinations: HashMap<_, _> = approved
        .rows
        .iter()
        .map(|row| (row.name.source.as_path(), row.name.destination.as_path()))
        .collect();
    let mut staged = moves[renamed..].iter();
    let mut removed = Vec::new();
    let copies_total = approved.duplicates.len();
    Phase::RemovingDuplicates.start(copies_total, &mut report);
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
        report(Phase::RemovingDuplicates.snapshot(removed.len(), copies_total));
    }
    let mut summary = ApplySummary {
        renamed,
        removed: removed.len(),
        directories_removed: 0,
        directory_errors: Vec::new(),
    };
    let directories_total = approved.directories.len();
    Phase::CheckingDirectories.start(directories_total, &mut report);
    for (index, (path, observation)) in approved.directories.iter().enumerate() {
        match source.remove_empty_directory(&approved.root, path, observation) {
            Ok(removed) => summary.directories_removed += usize::from(removed),
            Err(error) => summary.directory_errors.push(error),
        }
        report(Phase::CheckingDirectories.snapshot(index + 1, directories_total));
    }
    Ok(summary)
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
    report: &mut impl FnMut(Progress),
) -> io::Result<()> {
    let total = moves.len();
    Phase::Staging.start(total, report);
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
                    report(Phase::Staging.snapshot(index + 1, total));
                    break;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists && attempt < 15 => {}
                Err(error) => return Err(error),
            }
        }
    }

    let total = moves
        .iter()
        .filter(|movement| movement.destination.is_some())
        .count();
    Phase::Renaming.start(total, report);
    let changed = moves.iter_mut().filter_map(|movement| {
        movement
            .destination
            .map(|destination| (movement, destination))
    });
    for (index, (movement, destination)) in changed.enumerate() {
        source.move_exclusive(root, &movement.staged, destination, movement.observation)?;
        movement.location = Location::Final;
        report(Phase::Renaming.snapshot(index + 1, total));
    }
    Ok(())
}

fn restore(
    source: &impl RenameSource,
    root: &Path,
    moves: &mut [PendingMove<'_>],
    report: &mut impl FnMut(Progress),
) -> Vec<RecoveryFailure> {
    let total = moves
        .iter()
        .filter(|movement| movement.location != Location::Original)
        .count();
    Phase::Restoring.start(total, report);
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
    let pending = moves
        .iter_mut()
        .rev()
        .filter(|movement| movement.location != Location::Original);
    for (index, movement) in pending.enumerate() {
        let last_known = match movement.location {
            Location::Original => unreachable!(),
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
        report(Phase::Restoring.snapshot(index + 1, total));
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{LocalMediaSource, tests::Scratch};
    use std::fs;

    #[test]
    fn flatten_removes_affected_empty_ancestors_but_preserves_occupied_and_unrelated_directories() {
        let scratch = Scratch::new();
        let deep = scratch.0.join("affected/deep");
        let occupied = scratch.0.join("occupied");
        let unrelated = scratch.0.join("unrelated");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir(&occupied).unwrap();
        fs::create_dir(&unrelated).unwrap();
        fs::write(deep.join("a.JPG"), b"orig a").unwrap();
        fs::write(occupied.join("b.MOV"), b"orig b").unwrap();
        let options = OrganizationOptions {
            flatten: true,
            ..OrganizationOptions::default()
        };
        let plan = prepare_apply(&LocalMediaSource, &scratch.0, options).unwrap();
        fs::write(occupied.join("new.txt"), b"new occupant").unwrap();
        revalidate(&LocalMediaSource, &scratch.0, &plan).unwrap();
        let mut events = Vec::new();
        let result = execute(&LocalMediaSource, &plan, |event| events.push(event)).unwrap();
        assert_eq!(result.directories_removed, 2);
        assert!(events.ends_with(&[
            Phase::CheckingDirectories.snapshot(0, 3),
            Phase::CheckingDirectories.snapshot(1, 3),
            Phase::CheckingDirectories.snapshot(2, 3),
            Phase::CheckingDirectories.snapshot(3, 3),
        ]));
        assert_eq!(result.renamed, 2);
        assert!(!scratch.0.join("affected").exists());
        assert_eq!(fs::read(occupied.join("new.txt")).unwrap(), b"new occupant");
        assert!(unrelated.is_dir());
        let contents: Vec<_> = plan
            .rows
            .iter()
            .map(|row| fs::read(&row.name.destination).unwrap())
            .collect();
        assert!(contents.contains(&b"orig a".to_vec()));
        assert!(contents.contains(&b"orig b".to_vec()));
    }

    #[test]
    fn cleanup_uses_the_keeper_final_path() {
        let scratch = Scratch::new();
        let incoming = scratch.0.join("a");
        fs::create_dir(&incoming).unwrap();
        fs::write(incoming.join("a.JPG"), b"same").unwrap();
        fs::write(incoming.join("z.JPG"), b"same").unwrap();
        fs::write(scratch.0.join("other.JPG"), b"same").unwrap();
        fs::write(scratch.0.join("1.JPG"), b"diff").unwrap();

        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions {
                remove_duplicates: true,
                ..OrganizationOptions::default()
            },
        )
        .unwrap();
        let result = execute(&LocalMediaSource, &plan, |_| {}).unwrap();
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

        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions {
                remove_duplicates: true,
                ..OrganizationOptions::default()
            },
        )
        .unwrap();
        assert_eq!(plan.duplicates.len(), 1);
        let result = execute(&LocalMediaSource, &plan, |_| {}).unwrap();
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

        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions {
                remove_duplicates: true,
                ..OrganizationOptions::default()
            },
        )
        .unwrap();
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
        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions::default(),
        )
        .unwrap();
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
        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions::default(),
        )
        .unwrap();
        fs::create_dir(&destination).unwrap();
        assert!(revalidate(&LocalMediaSource, &scratch.0, &plan).is_err());

        let blocked = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions::default(),
        )
        .unwrap();
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
        let plan = prepare_apply(
            &LocalMediaSource,
            &scratch.0,
            OrganizationOptions::default(),
        )
        .unwrap();
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
        completed_destinations: RefCell<Vec<PathBuf>>,
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

        fn observe_directory(&self, _: &Path, _: &Path) -> io::Result<DirectoryObservation> {
            panic!("no observed directories in this synthetic plan");
        }

        fn remove_empty_directory(
            &self,
            _: &Path,
            _: &Path,
            _: &DirectoryObservation,
        ) -> io::Result<bool> {
            panic!("no directory cleanup in this synthetic plan");
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
            self.completed_destinations
                .borrow_mut()
                .push(destination.to_path_buf());
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
                completed_destinations: RefCell::new(Vec::new()),
                failures: RefCell::new(failures),
                cleanup_failure: None,
            },
            PreparedApply {
                root,
                report: AnalysisReport::default(),
                rows,
                duplicates: Vec::new(),
                options: OrganizationOptions::default(),
                directories: Vec::new(),
                sources,
            },
        )
    }

    #[test]
    fn finalization_failure_restores_staged_copies_without_unlinking() {
        let (adapter, mut plan) = sample(vec![(None, "root/1.jpg".into())]);
        add_copy(&adapter, &mut plan, "copy.jpg");
        let before = adapter.files.borrow().clone();

        let CleanupError::Rename(error) = execute(&adapter, &plan, |_| {}).unwrap_err() else {
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

        let mut events = Vec::new();
        let CleanupError::Removal {
            trigger,
            removed,
            remaining,
        } = execute(&adapter, &plan, |event| events.push(event)).unwrap_err()
        else {
            panic!("expected cleanup failure");
        };
        assert!(events.ends_with(&[
            Phase::RemovingDuplicates.snapshot(0, 3),
            Phase::RemovingDuplicates.snapshot(1, 3),
        ]));
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
        let mut events = Vec::new();
        assert_eq!(
            execute(&adapter, &plan, |event| events.push(event))
                .unwrap()
                .renamed,
            2
        );
        assert_eq!(
            events,
            [
                Phase::Staging.snapshot(0, 2),
                Phase::Staging.snapshot(1, 2),
                Phase::Staging.snapshot(2, 2),
                Phase::Renaming.snapshot(0, 2),
                Phase::Renaming.snapshot(1, 2),
                Phase::Renaming.snapshot(2, 2),
            ]
        );
        assert_eq!(
            *adapter.files.borrow(),
            [("root/1.jpg".into(), 2), ("root/2.jpg".into(), 1)].into()
        );
    }

    #[test]
    fn restores_already_finalized_and_staged_sources() {
        let (adapter, plan) = sample(vec![(None, "root/1.jpg".into())]);
        let CleanupError::Rename(error) = execute(&adapter, &plan, |_| {}).unwrap_err() else {
            panic!("expected rename failure");
        };
        assert!(error.recovery.is_empty());
        assert_eq!(
            *adapter.files.borrow(),
            [("root/1.jpg".into(), 1), ("root/2.jpg".into(), 2)].into()
        );
    }

    #[test]
    fn cross_parent_finalization_collision_restores_originals_and_preserves_occupant() {
        let (adapter, mut plan) = sample(Vec::new());
        plan.options.flatten = true;
        let first = plan.root.join("nested-a/first.jpg");
        let second = plan.root.join("nested-b/second.jpg");
        for (row, nested) in plan.rows.iter_mut().zip([&first, &second]) {
            let evidence = plan.sources.remove(&row.name.source).unwrap();
            plan.sources.insert(nested.clone(), evidence);
            let bytes = adapter.files.borrow_mut().remove(&row.name.source).unwrap();
            adapter.files.borrow_mut().insert(nested.clone(), bytes);
            row.name.source = nested.clone();
        }

        let first_destination = plan.rows[0].name.destination.clone();
        let occupied = plan.rows[1].name.destination.clone();
        adapter.files.borrow_mut().insert(occupied.clone(), 9);
        let CleanupError::Rename(error) = execute(&adapter, &plan, |_| {}).unwrap_err() else {
            panic!("expected a late destination collision");
        };
        assert_eq!(error.trigger.kind(), ErrorKind::AlreadyExists);
        assert!(error.recovery.is_empty());
        assert!(
            adapter
                .completed_destinations
                .borrow()
                .contains(&first_destination)
        );
        assert_eq!(
            *adapter.files.borrow(),
            [(first, 1), (second, 2), (occupied, 9)].into()
        );
    }

    #[test]
    fn staging_failure_restores_other_sources_and_keeps_occupied_stage_names() {
        let second = Path::new("root/2.jpg");
        let occupied = stage_path(second, 1, 0);
        let failed = stage_path(second, 1, 1);
        let (adapter, plan) = sample(vec![(Some(second.to_path_buf()), failed)]);
        adapter.files.borrow_mut().insert(occupied.clone(), 9);
        let CleanupError::Rename(error) = execute(&adapter, &plan, |_| {}).unwrap_err() else {
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
        let mut events = Vec::new();
        let CleanupError::Rename(error) =
            execute(&adapter, &plan, |event| events.push(event)).unwrap_err()
        else {
            panic!("expected rename failure");
        };
        assert_eq!(error.recovery.len(), 1);
        assert!(events.ends_with(&[
            Phase::Restoring.snapshot(0, 2),
            Phase::Restoring.snapshot(1, 2),
            Phase::Restoring.snapshot(2, 2),
        ]));
        let unresolved = &error.recovery[0];
        assert_eq!(unresolved.original, Path::new("root/1.jpg"));
        assert_ne!(unresolved.last_known, unresolved.original);
        assert_eq!(adapter.files.borrow()[&unresolved.last_known], 1);
        assert_eq!(adapter.files.borrow()[Path::new("root/2.jpg")], 2);
    }
}
