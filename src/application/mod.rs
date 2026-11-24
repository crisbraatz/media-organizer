//! Use cases for analyzing, planning and applying media organization.

use crate::domain::preview::{DatedMedia, ProposedName, propose_names};
use crate::domain::{AnalysisReport, MediaFormat};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) struct FolderInventory {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) entries: Vec<PathBuf>,
    pub(crate) subfolders: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Fingerprint {
    pub(crate) byte_count: u64,
    pub(crate) hash: u64,
}

pub(crate) trait MediaSource {
    fn inventory(&self, root: &Path, include_entries: bool) -> io::Result<FolderInventory>;
    fn capture_timestamp(&self, path: &Path) -> io::Result<Option<SystemTime>>;
    fn destination_exists(&self, path: &Path) -> io::Result<bool>;
    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint>;
    fn contents_equal(&self, first: &Path, second: &Path) -> io::Result<bool>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceObservation {
    pub(crate) len: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) created: Option<SystemTime>,
    #[cfg(unix)]
    pub(crate) device_inode: (u64, u64),
}

pub(crate) trait RenameSource {
    fn canonical_root(&self, root: &Path) -> io::Result<PathBuf>;
    fn observe_source(&self, root: &Path, source: &Path) -> io::Result<SourceObservation>;
    fn move_exclusive(
        &self,
        root: &Path,
        source: &Path,
        destination: &Path,
        observation: &SourceObservation,
    ) -> io::Result<()>;
}

pub(crate) struct Analysis {
    pub(crate) report: AnalysisReport,
    pub(crate) preview: Option<Vec<PreviewRow>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreviewRow {
    pub(crate) name: ProposedName,
    pub(crate) blocked: bool,
}

pub(crate) fn analyze_folder(
    source: &impl MediaSource,
    root: &Path,
    preview: bool,
) -> io::Result<Analysis> {
    analyze_with_observations(source, root, preview, None).map(|(analysis, _)| analysis)
}

fn analyze_with_observations(
    source: &impl MediaSource,
    root: &Path,
    preview: bool,
    observer: Option<&dyn RenameSource>,
) -> io::Result<(Analysis, HashMap<PathBuf, apply::SourceEvidence>)> {
    let mut inventory = source.inventory(root, preview)?;
    inventory.files.sort_unstable();

    let mut report = AnalysisReport {
        subfolders: inventory.subfolders,
        ..AnalysisReport::default()
    };
    let mut representatives: HashMap<Fingerprint, Vec<PathBuf>> = HashMap::new();
    let mut dated = Vec::new();
    let mut observations = HashMap::new();
    for path in inventory.files {
        let Some(format) = MediaFormat::classify(path.extension()) else {
            continue;
        };
        let before = observer
            .map(|observer| observer.observe_source(root, &path))
            .transpose()?;
        let fingerprint = source.fingerprint(&path)?;
        let bucket = representatives.entry(fingerprint).or_default();
        let mut duplicate = false;
        for other in bucket.iter() {
            if source.contents_equal(other, &path)? {
                duplicate = true;
                break;
            }
        }
        report.record(&format, duplicate);
        if preview {
            let timestamp = source.capture_timestamp(&path)?;
            if let Some((observer, observation)) = observer.zip(before) {
                if observer.observe_source(root, &path)? != observation {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Source changed during analysis: {path:?}"),
                    ));
                }
                observations.insert(
                    path.clone(),
                    apply::SourceEvidence {
                        observation,
                        fingerprint,
                        timestamp,
                    },
                );
            }
            dated.push(DatedMedia {
                path: path.clone(),
                timestamp,
            });
        }
        if !duplicate {
            bucket.push(path);
        }
    }

    let preview = if preview {
        Some(mark_conflicts(
            source,
            propose_names(dated),
            &inventory.entries,
        )?)
    } else {
        None
    };
    Ok((Analysis { report, preview }, observations))
}

fn mark_conflicts(
    source: &impl MediaSource,
    names: Vec<ProposedName>,
    entries: &[PathBuf],
) -> io::Result<Vec<PreviewRow>> {
    let exact: HashSet<_> = entries.iter().map(PathBuf::as_path).collect();
    let mut aliases = HashMap::new();
    for entry in &exact {
        if let Some(key) = alias_key(entry) {
            aliases
                .entry(key)
                .and_modify(|value| *value = None)
                .or_insert(Some(*entry));
        }
    }

    let movers: HashSet<_> = names
        .iter()
        .filter(|name| name.source != name.destination)
        .map(|name| name.source.as_path())
        .collect();
    let mut blocked = Vec::with_capacity(names.len());
    for name in &names {
        let target = name.destination.as_path();
        if name.source == target || !source.destination_exists(target)? {
            blocked.push(false);
            continue;
        }
        let occupant = exact
            .get(target)
            .copied()
            .or_else(|| alias_key(target).and_then(|key| aliases.get(&key).copied().flatten()));
        blocked.push(!occupant.is_some_and(|entry| entry == name.source || movers.contains(entry)));
    }

    Ok(names
        .into_iter()
        .zip(blocked)
        .map(|(name, blocked)| PreviewRow { name, blocked })
        .collect())
}

fn alias_key(path: &Path) -> Option<(&Path, Vec<u8>)> {
    Some((
        path.parent()?,
        path.file_name()?.as_encoded_bytes().to_ascii_lowercase(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct Source {
        files: Vec<PathBuf>,
        entries: Vec<PathBuf>,
        reads: Cell<usize>,
    }

    impl MediaSource for Source {
        fn inventory(&self, _: &Path, include_entries: bool) -> io::Result<FolderInventory> {
            Ok(FolderInventory {
                files: self.files.clone(),
                entries: if include_entries {
                    self.entries.clone()
                } else {
                    Vec::new()
                },
                subfolders: 0,
            })
        }

        fn fingerprint(&self, _: &Path) -> io::Result<Fingerprint> {
            Ok(Fingerprint {
                byte_count: 1,
                hash: 1,
            })
        }

        fn contents_equal(&self, _: &Path, _: &Path) -> io::Result<bool> {
            Ok(false)
        }

        fn capture_timestamp(&self, path: &Path) -> io::Result<Option<SystemTime>> {
            self.reads.set(self.reads.get() + 1);
            let secs = u64::from(path.file_stem().unwrap() == "1");
            Ok(Some(UNIX_EPOCH + Duration::from_secs(secs)))
        }

        fn destination_exists(&self, path: &Path) -> io::Result<bool> {
            Ok(self.entries.iter().any(|entry| {
                entry.parent() == path.parent()
                    && entry
                        .file_name()
                        .unwrap()
                        .as_encoded_bytes()
                        .eq_ignore_ascii_case(path.file_name().unwrap().as_encoded_bytes())
            }))
        }
    }

    fn source(files: &[&str], entries: &[&str]) -> Source {
        Source {
            files: files.iter().map(PathBuf::from).collect(),
            entries: entries.iter().map(PathBuf::from).collect(),
            reads: Cell::new(0),
        }
    }

    #[test]
    fn report_only_does_not_read_timestamps() {
        let source = source(&["root/photo.jpg"], &["root/photo.jpg"]);
        let analysis = analyze_folder(&source, Path::new("root"), false).unwrap();
        assert_eq!(analysis.report.total().files, 1);
        assert!(analysis.preview.is_none());
        assert_eq!(source.reads.get(), 0);
    }

    #[test]
    fn moving_occupants_free_a_name_but_exact_unmovable_entries_take_priority() {
        let source = source(
            &[
                "root/a/earlier.JPG",
                "root/a/1.jpg",
                "root/b/earlier.JPG",
                "root/b/1.jpg",
            ],
            &[
                "root/a/earlier.JPG",
                "root/a/1.jpg",
                "root/b/earlier.JPG",
                "root/b/1.jpg",
                "root/b/1.JPG",
            ],
        );
        let rows = analyze_folder(&source, Path::new("root"), true)
            .unwrap()
            .preview
            .unwrap();
        assert_eq!(rows.len(), 4);
        assert!(!rows[0].blocked);
        assert!(rows[2].blocked);
    }

    #[test]
    fn unchanged_source_never_blocks_itself() {
        let source = source(&["root/1.JPG"], &["root/1.JPG"]);
        let rows = analyze_folder(&source, Path::new("root"), true)
            .unwrap()
            .preview
            .unwrap();
        assert_eq!(rows[0].name.source, rows[0].name.destination);
        assert!(!rows[0].blocked);
    }
}

pub(crate) mod apply;
