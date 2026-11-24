//! Filesystem and media metadata adapters.

use crate::application::{Fingerprint, FolderInventory, MediaSource};
use std::fs::{self, File, Metadata};
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

const BLOCK_SIZE: usize = 16 * 1024;

pub(crate) struct LocalMediaSource;

impl MediaSource for LocalMediaSource {
    fn inventory(&self, root: &Path, include_entries: bool) -> io::Result<FolderInventory> {
        let mut pending = vec![root.to_path_buf()];
        let mut inventory = FolderInventory {
            files: Vec::new(),
            entries: Vec::new(),
            subfolders: 0,
        };
        while let Some(directory) = pending.pop() {
            let entries = fs::read_dir(&directory)
                .map_err(|error| path_error("read folder", &directory, error))?;
            for entry in entries {
                let entry =
                    entry.map_err(|error| path_error("read entry in", &directory, error))?;
                let path = entry.path();
                if include_entries {
                    inventory.entries.push(path.clone());
                }
                let metadata = entry
                    .metadata()
                    .map_err(|error| path_error("inspect", &path, error))?;
                if is_link_or_reparse_point(&metadata) {
                    continue;
                }
                if metadata.is_dir() {
                    inventory.subfolders += 1;
                    pending.push(path);
                } else if metadata.is_file() {
                    inventory.files.push(path);
                }
            }
        }
        Ok(inventory)
    }

    fn capture_timestamp(&self, path: &Path) -> io::Result<Option<SystemTime>> {
        capture_time::capture_timestamp(path)
    }

    fn destination_exists(&self, path: &Path) -> io::Result<bool> {
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(path_error("inspect proposed destination", path, error)),
        }
    }

    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
        let mut file = open_file(path)?;
        fingerprint(&mut file).map_err(|error| path_error("read", path, error))
    }

    fn contents_equal(&self, first: &Path, second: &Path) -> io::Result<bool> {
        let mut left = open_file(first)?;
        let mut right = open_file(second)?;
        // Label each stream so a failure names the affected file.
        contents_equal(
            &mut PathReader {
                file: &mut left,
                path: first,
            },
            &mut PathReader {
                file: &mut right,
                path: second,
            },
        )
    }
}

#[cfg(windows)]
fn is_link_or_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.is_symlink() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse_point(metadata: &Metadata) -> bool {
    metadata.is_symlink()
}

fn open_file(path: &Path) -> io::Result<File> {
    File::open(path).map_err(|error| path_error("open", path, error))
}

fn path_error(action: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("could not {action} {path:?}: {error}"),
    )
}

struct PathReader<'a> {
    file: &'a mut File,
    path: &'a Path,
}

impl Read for PathReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file
            .read(buffer)
            .map_err(|error| path_error("read", self.path, error))
    }
}

fn fingerprint(reader: &mut impl Read) -> io::Result<Fingerprint> {
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0; BLOCK_SIZE];
    let mut byte_count = 0;
    loop {
        let filled = read_block(reader, &mut buffer)?;
        if filled == 0 {
            return Ok(Fingerprint {
                byte_count,
                hash: hasher.finish(),
            });
        }
        hasher.write(&buffer[..filled]);
        byte_count += filled as u64;
    }
}

fn contents_equal(left: &mut impl Read, right: &mut impl Read) -> io::Result<bool> {
    let mut left_buffer = [0; BLOCK_SIZE];
    let mut right_buffer = [0; BLOCK_SIZE];
    loop {
        let left_len = read_block(left, &mut left_buffer)?;
        let right_len = read_block(right, &mut right_buffer)?;
        if left_buffer[..left_len] != right_buffer[..right_len] {
            return Ok(false);
        }
        if left_len < BLOCK_SIZE {
            return Ok(true);
        }
    }
}

fn read_block(reader: &mut impl Read, mut remaining: &mut [u8]) -> io::Result<usize> {
    let capacity = remaining.len();
    while !remaining.is_empty() {
        match reader.read(remaining) {
            Ok(0) => break,
            Ok(bytes_read) => remaining = &mut remaining[bytes_read..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(capacity - remaining.len())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::application::{OrganizationOptions, analyze_folder};
    use crate::domain::MediaCounts;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub(crate) struct Scratch(pub(crate) PathBuf);

    impl Scratch {
        pub(crate) fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "media-organizer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn counts_nested_media_and_extra_copies_across_extensions() {
        let root = Scratch::new();
        let nested = root.0.join("nested");
        std::fs::create_dir_all(nested.join("deeper")).unwrap();
        std::fs::write(root.0.join("a.jpg"), b"copy").unwrap();
        std::fs::write(root.0.join("b.JPG"), b"copy").unwrap();
        std::fs::write(nested.join("deeper/c.mov"), b"copy").unwrap();
        std::fs::write(nested.join("different.png"), b"else").unwrap();
        std::fs::write(nested.join("ignored.txt"), b"copy").unwrap();

        let report = analyze_folder(
            &LocalMediaSource,
            &root.0,
            false,
            OrganizationOptions::default(),
        )
        .unwrap()
        .report;

        assert_eq!(report.subfolders, 2);
        assert_eq!(
            report.total(),
            MediaCounts {
                files: 4,
                duplicates: 2
            }
        );
        assert_eq!(report.extensions["jpg"].duplicates, 1);
        assert_eq!(report.extensions["mov"].duplicates, 1);
    }

    #[cfg(unix)]
    #[test]
    fn skips_child_links() {
        let root = Scratch::new();
        let outside = Scratch::new();
        let media = outside.0.join("outside.jpg");
        std::fs::write(&media, b"outside").unwrap();
        std::os::unix::fs::symlink(&outside.0, root.0.join("directory-link")).unwrap();
        std::os::unix::fs::symlink(media, root.0.join("file-link.jpg")).unwrap();

        let report = analyze_folder(
            &LocalMediaSource,
            &root.0,
            false,
            OrganizationOptions::default(),
        )
        .unwrap()
        .report;

        assert_eq!(report.subfolders, 0);
        assert_eq!(report.total(), MediaCounts::default());
    }

    #[test]
    fn reports_path_for_failed_operations() {
        let root = Scratch::new();
        let absent = root.0.join("absent.jpg");

        let Err(error) = analyze_folder(
            &LocalMediaSource,
            &absent,
            false,
            OrganizationOptions::default(),
        ) else {
            panic!("missing directory was accepted");
        };
        assert!(error.to_string().contains("absent.jpg"));
        assert!(
            LocalMediaSource
                .fingerprint(&absent)
                .unwrap_err()
                .to_string()
                .contains("absent.jpg")
        );
    }

    struct Fragmented<'a>(Cursor<&'a [u8]>, usize);

    impl Read for Fragmented<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let fragment = buffer.len().min(self.1);
            self.0.read(&mut buffer[..fragment])
        }
    }

    #[test]
    fn short_reads_do_not_change_fingerprints_or_equality() {
        let bytes = vec![5; BLOCK_SIZE + 3];
        let mut left = Fragmented(Cursor::new(bytes.as_slice()), 17);
        let mut right = Fragmented(Cursor::new(bytes.as_slice()), 31);
        assert_eq!(
            fingerprint(&mut left).unwrap(),
            fingerprint(&mut right).unwrap()
        );

        let mut left = Fragmented(Cursor::new(bytes.as_slice()), 17);
        let mut right = Fragmented(Cursor::new(bytes.as_slice()), 31);
        assert!(contents_equal(&mut left, &mut right).unwrap());

        let mut left = Fragmented(Cursor::new(bytes.as_slice()), 17);
        let mut right = Fragmented(Cursor::new(&bytes[..BLOCK_SIZE + 2]), 31);
        assert!(!contents_equal(&mut left, &mut right).unwrap());
    }
}

pub(crate) mod capture_time;

mod rename;
