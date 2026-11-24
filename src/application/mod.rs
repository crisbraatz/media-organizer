//! Use cases for analyzing, planning and applying media organization.

use crate::domain::{AnalysisReport, MediaFormat};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct FolderInventory {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) subfolders: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Fingerprint {
    pub(crate) byte_count: u64,
    pub(crate) hash: u64,
}

pub(crate) trait MediaSource {
    fn inventory(&self, root: &Path) -> io::Result<FolderInventory>;
    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint>;
    fn contents_equal(&self, first: &Path, second: &Path) -> io::Result<bool>;
}

pub(crate) fn analyze(source: &impl MediaSource, root: &Path) -> io::Result<AnalysisReport> {
    let mut inventory = source.inventory(root)?;
    inventory.files.sort_unstable();

    let mut report = AnalysisReport {
        subfolders: inventory.subfolders,
        ..AnalysisReport::default()
    };
    let mut representatives: HashMap<Fingerprint, Vec<PathBuf>> = HashMap::new();
    for path in inventory.files {
        let Some(format) = MediaFormat::classify(path.extension()) else {
            continue;
        };
        let bucket = representatives
            .entry(source.fingerprint(&path)?)
            .or_default();
        let mut duplicate = false;
        for other in bucket.iter() {
            if source.contents_equal(other, &path)? {
                duplicate = true;
                break;
            }
        }
        report.record(&format, duplicate);
        if !duplicate {
            bucket.push(path);
        }
    }

    Ok(report)
}
