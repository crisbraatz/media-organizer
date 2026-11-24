use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) struct DatedMedia {
    pub(crate) path: PathBuf,
    pub(crate) timestamp: Option<SystemTime>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProposedName {
    pub(crate) source: PathBuf,
    pub(crate) destination: PathBuf,
}

pub(crate) fn propose_names(
    items: Vec<DatedMedia>,
    root_destination: Option<&Path>,
) -> Vec<ProposedName> {
    let mut folders = BTreeMap::<PathBuf, Vec<DatedMedia>>::new();
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        let folder = root_destination
            .or_else(|| item.path.parent())
            .map(|path| path.to_path_buf())
            .unwrap_or_default();
        folders.entry(folder).or_default().push(item);
    }

    for (folder, mut media) in folders {
        media.sort_unstable_by(|left, right| {
            left.timestamp
                .is_none()
                .cmp(&right.timestamp.is_none())
                .then_with(|| left.timestamp.cmp(&right.timestamp))
                .then_with(|| left.path.file_name().cmp(&right.path.file_name()))
                .then_with(|| left.path.cmp(&right.path))
        });

        let width = media.len().to_string().len();
        for (offset, item) in media.into_iter().enumerate() {
            let index = offset + 1;
            let mut destination = folder.join(format!("{index:0width$}"));
            if let Some(extension) = item.path.extension() {
                destination.set_extension(extension);
            }
            rows.push(ProposedName {
                source: item.path,
                destination,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn dated(path: impl Into<PathBuf>, seconds: Option<u64>) -> DatedMedia {
        DatedMedia {
            path: path.into(),
            timestamp: seconds.map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds)),
        }
    }

    #[test]
    fn flattened_names_share_root_and_break_equal_name_ties_by_path() {
        let rows = propose_names(
            vec![
                dated("root/b/photo.JPG", Some(2)),
                dated("root/a/photo.JPG", Some(2)),
                dated("root/later.mov", None),
                dated("root/sub/early.png", Some(1)),
            ],
            Some(std::path::Path::new("root")),
        );

        assert_eq!(rows[0].destination, PathBuf::from("root/1.png"));
        assert_eq!(rows[1].source, PathBuf::from("root/a/photo.JPG"));
        assert_eq!(rows[1].destination, PathBuf::from("root/2.JPG"));
        assert_eq!(rows[2].destination, PathBuf::from("root/3.JPG"));
        assert_eq!(rows[3].destination, PathBuf::from("root/4.mov"));
    }

    #[test]
    fn orders_by_time_then_native_name_and_places_unknown_dates_last() {
        let rows = propose_names(
            vec![
                dated("parent/early.JPG", Some(5)),
                dated("parent/b.jpg", Some(8)),
                dated("parent/a.jpg", Some(8)),
                dated("parent/0.jpg", None),
                dated("parent/1.jpg", None),
            ],
            None,
        );

        let pairs: Vec<_> = rows
            .into_iter()
            .map(|row| (row.source, row.destination))
            .collect();
        assert_eq!(
            pairs,
            [
                ("parent/early.JPG".into(), "parent/1.JPG".into()),
                ("parent/a.jpg".into(), "parent/2.jpg".into()),
                ("parent/b.jpg".into(), "parent/3.jpg".into()),
                ("parent/0.jpg".into(), "parent/4.jpg".into()),
                ("parent/1.jpg".into(), "parent/5.jpg".into()),
            ]
        );
    }

    #[test]
    fn numbers_each_directory_independently_and_includes_unchanged_rows() {
        let mut items = vec![dated("a/1.JPG", Some(1)), dated("a/sub/untitled.mov", None)];
        for index in 1..=98 {
            items.push(dated(format!("b/img{index}.jpg"), Some(index)));
        }
        let rows = propose_names(items, None);

        assert_eq!(rows.len(), 100);
        assert_eq!(rows[0].source, rows[0].destination);
        assert_eq!(rows[1].destination, PathBuf::from("a/sub/1.mov"));
        assert_eq!(rows[2].destination, PathBuf::from("b/01.jpg"));
        assert_eq!(rows[99].destination, PathBuf::from("b/98.jpg"));
    }
}
