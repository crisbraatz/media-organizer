//! Rules for chronological ordering, duplicates and sequence naming.

use std::collections::BTreeMap;
use std::ffi::OsStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaKind {
    Photo,
    Video,
}

pub(crate) struct MediaFormat {
    pub(crate) extension: String,
    pub(crate) kind: MediaKind,
}

impl MediaFormat {
    pub(crate) fn classify(extension: Option<&OsStr>) -> Option<Self> {
        let extension = extension?.to_str()?.to_ascii_lowercase();
        let kind = match extension.as_str() {
            "heic" | "heif" | "jpg" | "jpeg" | "png" | "webp" | "gif" | "tif" | "tiff" | "bmp"
            | "dng" | "arw" | "cr2" | "cr3" | "nef" | "raf" | "orf" | "rw2" => MediaKind::Photo,
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "wmv" | "mpeg" | "mpg" | "mts"
            | "3gp" => MediaKind::Video,
            _ => return None,
        };
        Some(Self { extension, kind })
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct MediaCounts {
    pub(crate) files: usize,
    pub(crate) duplicates: usize,
}

impl MediaCounts {
    fn record(&mut self, duplicate: bool) {
        self.files += 1;
        self.duplicates += usize::from(duplicate);
    }
}

#[derive(Default)]
pub(crate) struct AnalysisReport {
    pub(crate) photos: MediaCounts,
    pub(crate) videos: MediaCounts,
    pub(crate) extensions: BTreeMap<String, MediaCounts>,
    pub(crate) subfolders: usize,
}

impl AnalysisReport {
    pub(crate) fn total(&self) -> MediaCounts {
        MediaCounts {
            files: self.photos.files + self.videos.files,
            duplicates: self.photos.duplicates + self.videos.duplicates,
        }
    }

    pub(crate) fn record(&mut self, format: &MediaFormat, duplicate: bool) {
        let counts = match format.kind {
            MediaKind::Photo => &mut self.photos,
            MediaKind::Video => &mut self.videos,
        };
        counts.record(duplicate);
        self.extensions
            .entry(format.extension.clone())
            .or_default()
            .record(duplicate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn classifies_supported_extensions_case_insensitively() {
        for extension in [
            "heic", "heif", "jpg", "jpeg", "png", "webp", "gif", "tif", "tiff", "bmp", "dng",
            "arw", "cr2", "cr3", "nef", "raf", "orf", "rw2",
        ] {
            let upper = extension.to_ascii_uppercase();
            let format = MediaFormat::classify(Some(OsStr::new(&upper))).unwrap();
            assert_eq!(format.kind, MediaKind::Photo);
            assert_eq!(format.extension, extension);
        }

        for extension in [
            "mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "mpeg", "mpg", "mts", "3gp",
        ] {
            let upper = extension.to_ascii_uppercase();
            let format = MediaFormat::classify(Some(OsStr::new(&upper))).unwrap();
            assert_eq!(format.kind, MediaKind::Video);
            assert_eq!(format.extension, extension);
        }

        for extension in [None, Some(OsStr::new("")), Some(OsStr::new("txt"))] {
            assert!(MediaFormat::classify(extension).is_none());
        }
    }

    #[test]
    fn reports_zero_counts_and_orders_extensions() {
        let mut report = AnalysisReport::default();
        assert_eq!(report.total(), MediaCounts::default());
        assert!(report.extensions.is_empty());
        assert_eq!(report.subfolders, 0);

        let png = MediaFormat::classify(Some(OsStr::new("PNG"))).unwrap();
        let jpg = MediaFormat::classify(Some(OsStr::new("jpg"))).unwrap();
        let mp4 = MediaFormat::classify(Some(OsStr::new("mp4"))).unwrap();
        report.record(&png, false);
        report.record(&jpg, true);
        report.record(&mp4, true);

        assert_eq!(
            report.total(),
            MediaCounts {
                files: 3,
                duplicates: 2
            }
        );
        assert_eq!(
            report.photos,
            MediaCounts {
                files: 2,
                duplicates: 1
            }
        );
        assert_eq!(
            report.videos,
            MediaCounts {
                files: 1,
                duplicates: 1
            }
        );
        assert_eq!(
            report
                .extensions
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["jpg", "mp4", "png"],
        );
        assert_eq!(report.extensions["jpg"].duplicates, 1);
    }
}

pub(crate) mod preview;
