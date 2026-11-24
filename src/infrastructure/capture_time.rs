use std::io::{self, Read, Seek};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nom_exif::{
    EntryValue, Error, Exif, ExifDateTime, ExifTag, MediaKind, MediaParser, MediaSource,
    TrackInfoTag,
};

pub(crate) fn capture_timestamp(path: &Path) -> io::Result<Option<SystemTime>> {
    let mut file = super::open_file(path)?;
    if let Some(timestamp) = read_capture(&mut file)
        .map_err(|error| super::path_error("read capture metadata from", path, error))?
    {
        return Ok(Some(timestamp));
    }

    let metadata = file
        .metadata()
        .map_err(|error| super::path_error("inspect", path, error))?;
    from_creation(metadata.created())
        .map_err(|error| super::path_error("read creation time for", path, error))
}

fn from_creation(result: io::Result<SystemTime>) -> io::Result<Option<SystemTime>> {
    match result {
        Ok(time) => Ok(Some(time)),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_capture(reader: impl Read + Seek) -> io::Result<Option<SystemTime>> {
    match parse_capture(reader) {
        Ok(timestamp) => Ok(timestamp),
        // A truncated metadata block can arrive as either parser EOF or I/O EOF.
        Err(Error::Io(error)) if error.kind() != io::ErrorKind::UnexpectedEof => Err(error),
        Err(_) => Ok(None),
    }
}

fn parse_capture(reader: impl Read + Seek) -> Result<Option<SystemTime>, Error> {
    let source = MediaSource::seekable(reader)?;
    let mut parser = MediaParser::new();
    match source.kind() {
        MediaKind::Image => {
            let exif = Exif::from(parser.parse_exif(source)?);
            Ok(exif.get(ExifTag::DateTimeOriginal).and_then(as_system_time))
        }
        MediaKind::Track => {
            // A zero QuickTime movie-header creation date is the 1904 epoch.
            let quicktime_epoch = UNIX_EPOCH - Duration::from_secs(2_082_844_800);
            let track = parser.parse_track(source)?;
            Ok(track
                .get(TrackInfoTag::CreateDate)
                .and_then(as_system_time)
                .filter(|&time| time != quicktime_epoch))
        }
    }
}

fn as_system_time(value: &EntryValue) -> Option<SystemTime> {
    Some(match value.as_datetime()? {
        ExifDateTime::Aware(time) => time.into(),
        ExifDateTime::Naive(time) => time.and_utc().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, SeekFrom};

    const EXPECTED_UNIX_TIME: u64 = 1_704_067_200; // 2024-01-01 00:00:00 UTC

    fn expected_time() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(EXPECTED_UNIX_TIME)
    }

    fn capture_tiff(with_offset: bool, dng: bool) -> Vec<u8> {
        let mut data = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
        let exif_ifd_offset = if dng { 38_u32 } else { 26 };
        data.extend((u16::from(dng) + 1).to_le_bytes());
        data.extend([0x69, 0x87, 4, 0, 1, 0, 0, 0]);
        data.extend(exif_ifd_offset.to_le_bytes());
        if dng {
            // DNGVersion, stored inline as four bytes.
            data.extend([0x12, 0xc6, 1, 0, 4, 0, 0, 0, 1, 4, 0, 0]);
        }
        data.extend([0; 4]); // No next IFD.
        data.extend((u16::from(with_offset) + 1).to_le_bytes());
        let stamp_offset = exif_ifd_offset + 18 + u32::from(with_offset) * 12;
        data.extend([0x03, 0x90, 2, 0, 20, 0, 0, 0]);
        data.extend(stamp_offset.to_le_bytes());
        if with_offset {
            data.extend([0x11, 0x90, 2, 0, 7, 0, 0, 0]);
            data.extend((stamp_offset + 20).to_le_bytes());
        }
        data.extend([0; 4]);
        if with_offset {
            data.extend(b"2024:01:01 02:00:00\0+02:00\0");
        } else {
            data.extend(b"2024:01:01 00:00:00\0");
        }
        data
    }

    fn jpeg() -> Vec<u8> {
        let tiff = capture_tiff(true, false);
        let mut data = vec![0xff, 0xd8, 0xff, 0xe1];
        data.extend(u16::try_from(tiff.len() + 8).unwrap().to_be_bytes());
        data.extend(b"Exif\0\0");
        data.extend(tiff);
        data.extend([0xff, 0xe2, 0x24, 0x02]);
        data.extend([0; 0x2400]);
        data.extend([0xff, 0xd9]);
        data
    }

    fn atom(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut atom = u32::try_from(body.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        atom.extend(name);
        atom.extend(body);
        atom
    }

    fn mp4(unix_timestamp: Option<u64>) -> Vec<u8> {
        let mut mvhd = vec![0; 100];
        let timestamp = unix_timestamp.map_or(0, |t| u32::try_from(t + 2_082_844_800).unwrap());
        mvhd[4..8].copy_from_slice(&timestamp.to_be_bytes());
        mvhd[12..16].copy_from_slice(&1000_u32.to_be_bytes());
        let mut data = atom(b"ftyp", b"isom\0\0\0\0isom");
        data.extend(atom(b"moov", &atom(b"mvhd", &mvhd)));
        data.extend(atom(b"free", &[0; 0x2400]));
        data
    }

    #[test]
    fn parses_generated_jpeg_dng_and_mp4_capture_times() {
        let mut dng = capture_tiff(false, true);
        dng.resize(0x2400, 0);
        for (label, bytes) in [
            ("JPEG", jpeg()),
            ("DNG", dng),
            ("MP4", mp4(Some(EXPECTED_UNIX_TIME))),
        ] {
            assert_eq!(
                read_capture(Cursor::new(bytes)).unwrap(),
                Some(expected_time()),
                "{label}"
            );
        }
    }

    #[test]
    fn missing_malformed_and_default_metadata_are_unavailable() {
        for bytes in [
            vec![],
            jpeg()[..26].to_vec(),
            mp4(None),
            b"unsupported".to_vec(),
        ] {
            assert_eq!(read_capture(Cursor::new(bytes)).unwrap(), None);
        }
        assert_eq!(
            from_creation(Err(io::ErrorKind::Unsupported.into())).unwrap(),
            None
        );
        assert!(from_creation(Err(io::ErrorKind::PermissionDenied.into())).is_err());
    }

    #[test]
    fn capture_time_precedes_creation_time_and_missing_metadata_falls_back() {
        struct Fixture(std::path::PathBuf);

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        let path = std::env::temp_dir().join(format!(
            "media-organizer-capture-{}.jpg",
            std::process::id()
        ));
        let mut file = std::fs::File::create_new(&path).unwrap();
        let fixture = Fixture(path);
        std::io::Write::write_all(&mut file, &jpeg()).unwrap();
        drop(file);
        assert_eq!(
            capture_timestamp(&fixture.0).unwrap(),
            Some(expected_time())
        );

        std::fs::write(&fixture.0, b"no EXIF").unwrap();
        let expected = from_creation(std::fs::metadata(&fixture.0).unwrap().created()).unwrap();
        assert_eq!(capture_timestamp(&fixture.0).unwrap(), expected);
    }

    #[test]
    fn read_failures_are_reported() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        impl Seek for Broken {
            fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
                Ok(0)
            }
        }
        assert_eq!(
            read_capture(Broken).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}
