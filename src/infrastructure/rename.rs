use super::{LocalMediaSource, is_link_or_reparse_point, path_error};
use crate::application::{
    DuplicateRemoval, MediaSource, RenameSource, SourceObservation, StagedCopy,
};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

impl RenameSource for LocalMediaSource {
    fn canonical_root(&self, root: &Path) -> io::Result<PathBuf> {
        let canonical = fs::canonicalize(root)
            .map_err(|error| path_error("resolve the selected folder", root, error))?;
        let metadata = checked_metadata(&canonical)?;
        if !metadata.is_dir() {
            return Err(path_error(
                "use as selected folder",
                root,
                ErrorKind::NotADirectory.into(),
            ));
        }
        Ok(canonical)
    }

    fn observe_source(&self, root: &Path, source: &Path) -> io::Result<SourceObservation> {
        let parent = source
            .parent()
            .filter(|_| source.file_name().is_some())
            .filter(|parent| root.is_absolute() && parent.starts_with(root))
            .ok_or_else(|| {
                path_error(
                    "access within the selected folder",
                    source,
                    ErrorKind::InvalidInput.into(),
                )
            })?;
        let resolved = fs::canonicalize(parent)
            .map_err(|error| path_error("resolve parent folder", parent, error))?;
        if resolved != parent || !checked_metadata(parent)?.is_dir() {
            return Err(path_error(
                "use changed parent folder",
                parent,
                ErrorKind::InvalidInput.into(),
            ));
        }

        let metadata = checked_metadata(source)?;
        if !metadata.is_file() {
            return Err(path_error(
                "use as a regular source file",
                source,
                ErrorKind::InvalidInput.into(),
            ));
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        Ok(SourceObservation {
            len: metadata.len(),
            modified: optional_time(metadata.modified(), source)?,
            created: optional_time(metadata.created(), source)?,
            #[cfg(unix)]
            device_inode: (metadata.dev(), metadata.ino()),
        })
    }

    fn move_exclusive(
        &self,
        root: &Path,
        source: &Path,
        destination: &Path,
        observation: &SourceObservation,
    ) -> io::Result<()> {
        if source.parent() != destination.parent() || destination.file_name().is_none() {
            return Err(path_error(
                "use as a sibling destination",
                destination,
                ErrorKind::InvalidInput.into(),
            ));
        }
        if !matches_for_move(&self.observe_source(root, source)?, observation) {
            return Err(path_error(
                "move changed source",
                source,
                ErrorKind::InvalidData.into(),
            ));
        }
        renamest::rename_exclusive(source, destination).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not rename {source:?} to {destination:?}: {error}"),
            )
        })
    }
}

impl DuplicateRemoval for LocalMediaSource {
    fn remove_staged_copy(&self, root: &Path, copy: StagedCopy<'_>) -> io::Result<()> {
        if copy.path == copy.keeper
            || copy.path.extension() != Some("stage".as_ref())
            || !copy
                .path
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b".media-organizer-"))
        {
            return Err(path_error(
                "remove as staged copy",
                copy.path,
                ErrorKind::InvalidInput.into(),
            ));
        }
        let observe_pair = || -> io::Result<()> {
            for (path, reviewed) in [
                (copy.path, copy.observation),
                (copy.keeper, copy.keeper_observation),
            ] {
                if !matches_for_move(&self.observe_source(root, path)?, reviewed) {
                    return Err(path_error(
                        "remove copy with changed entry",
                        path,
                        ErrorKind::InvalidData.into(),
                    ));
                }
            }
            Ok(())
        };
        observe_pair()?;
        if !self.contents_equal(copy.path, copy.keeper)? {
            return Err(path_error(
                "remove copy with different content",
                copy.path,
                ErrorKind::InvalidData.into(),
            ));
        }
        observe_pair()?;
        fs::remove_file(copy.path)
            .map_err(|error| path_error("remove staged copy", copy.path, error))
    }
}

fn matches_for_move(live: &SourceObservation, reviewed: &SourceObservation) -> bool {
    if cfg!(windows) {
        // Windows file-system tunneling can change creation time when we reuse a name.
        live.len == reviewed.len && live.modified == reviewed.modified
    } else {
        live == reviewed
    }
}

fn checked_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| path_error("inspect", path, error))?;
    if is_link_or_reparse_point(&metadata) {
        return Err(path_error(
            "use link or reparse point",
            path,
            ErrorKind::InvalidInput.into(),
        ));
    }
    Ok(metadata)
}

fn optional_time(result: io::Result<SystemTime>, path: &Path) -> io::Result<Option<SystemTime>> {
    match result {
        Ok(time) => Ok(Some(time)),
        Err(error) if error.kind() == ErrorKind::Unsupported => Ok(None),
        Err(error) => Err(path_error("read timestamp of", path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::{LocalMediaSource, tests::Scratch};

    #[test]
    fn refuses_cleanup_if_reviewed_entries_have_different_bytes() {
        let scratch = Scratch::new();
        let fs = LocalMediaSource;
        let root = fs.canonical_root(&scratch.0).unwrap();
        let copy = root.join(".media-organizer-test.stage");
        let keeper = root.join("keeper.JPG");
        fs::write(&copy, b"copy").unwrap();
        fs::write(&keeper, b"keep").unwrap();

        let request = StagedCopy {
            path: &copy,
            observation: &fs.observe_source(&root, &copy).unwrap(),
            keeper: &keeper,
            keeper_observation: &fs.observe_source(&root, &keeper).unwrap(),
        };
        assert_eq!(
            fs.remove_staged_copy(&root, request).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&copy).unwrap(), b"copy");
        assert_eq!(fs::read(&keeper).unwrap(), b"keep");
    }

    #[test]
    fn refuses_occupied_targets_and_preserves_bytes_when_renaming() {
        let scratch = Scratch::new();
        let fs = LocalMediaSource;
        let root = fs.canonical_root(&scratch.0).unwrap();
        let source = root.join("image.jpg");
        let target = root.join("1.jpg");
        fs::write(&source, b"original").unwrap();
        fs::write(&target, b"occupant").unwrap();
        let observation = fs.observe_source(&root, &source).unwrap();

        assert!(
            fs.move_exclusive(&root, &source, &target, &observation)
                .is_err()
        );
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert_eq!(fs::read(&target).unwrap(), b"occupant");

        let directory = root.join("directory.jpg");
        fs::create_dir(&directory).unwrap();
        assert!(
            fs.move_exclusive(&root, &source, &directory, &observation)
                .is_err()
        );
        assert!(directory.is_dir());

        let free = root.join("2.jpg");
        fs.move_exclusive(&root, &source, &free, &observation)
            .unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&free).unwrap(), b"original");
    }

    #[test]
    fn rejects_outside_and_non_regular_sources() {
        let scratch = Scratch::new();
        let outside = Scratch::new();
        let fs = LocalMediaSource;
        let root = fs.canonical_root(&scratch.0).unwrap();
        let outside = fs.canonical_root(&outside.0).unwrap();
        let source = root.join("image.jpg");
        fs::write(&source, b"original").unwrap();
        let observation = fs.observe_source(&root, &source).unwrap();

        let external = outside.join("external.jpg");
        fs::write(&external, b"external").unwrap();
        assert_eq!(
            fs.observe_source(&root, &external).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(
            fs.move_exclusive(&root, &source, &outside.join("1.jpg"), &observation)
                .is_err()
        );
        assert_eq!(fs::read(&source).unwrap(), b"original");

        let directory = root.join("directory.jpg");
        fs::create_dir(&directory).unwrap();
        assert!(fs.observe_source(&root, &directory).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_sources() {
        let scratch = Scratch::new();
        let fs = LocalMediaSource;
        let root = fs.canonical_root(&scratch.0).unwrap();
        let source = root.join("image.jpg");
        fs::write(&source, b"original").unwrap();
        let link = root.join("linked.jpg");
        std::os::unix::fs::symlink(&source, &link).unwrap();

        assert!(fs.observe_source(&root, &link).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"original");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn per_move_matching_tolerates_creation_time_changes_only() {
        let reviewed = SourceObservation {
            len: 5,
            modified: Some(UNIX_EPOCH),
            created: Some(UNIX_EPOCH),
        };
        let mut live = reviewed.clone();
        live.created = Some(UNIX_EPOCH + Duration::from_secs(1));
        assert_ne!(live, reviewed);
        assert!(matches_for_move(&live, &reviewed));

        live.len += 1;
        assert!(!matches_for_move(&live, &reviewed));
        live.len = reviewed.len;
        live.modified = None;
        assert!(!matches_for_move(&live, &reviewed));
    }
}
