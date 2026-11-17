//! Refuses `-wal` sidecars whose page size disagrees with the database.
//!
//! libSQL 0.9.30 bundles `SQLite` 3.45.1. Before 3.54.0, `SQLite` leaves the
//! tail of a page buffer uninitialized when it reads a WAL frame that is
//! smaller than the pager's page size, so crafted database and `-wal` files
//! can make ordinary queries return stale process memory (CVE-2026-119375,
//! RUSTSEC-2026-0459, #940). The pager takes its page size from the database
//! header and, after reading page 1, from the newest copy of page 1, which
//! may be in the WAL. A legitimate WAL always uses the database's page size,
//! because the page size cannot change while a database is in WAL mode.
//!
//! [`check`] therefore refuses a WAL that `SQLite` would replay if its page
//! size differs from the database header or from any copy of page 1 it
//! holds. WAL files that `SQLite` ignores (shorter than a header, or with an
//! invalid header) pass, as does a missing or empty database, whose WAL
//! `SQLite` deletes on open.
//!
//! The check runs before `SQLite` opens the files. It does not protect
//! against a process that rewrites the sidecar after the check: the threat
//! is crafted files at rest, such as a `.tokensave/` directory shipped in a
//! repository. Remove this module once the bundled `SQLite` has the upstream
//! fix.

use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The 16-byte string that starts every `SQLite` database header.
const DB_MAGIC: &[u8; 16] = b"SQLite format 3\0";
/// The header prefix read from a database page: magic plus page size.
const DB_HEADER_LEN: usize = 18;
const WAL_HEADER_LEN: usize = 32;
const WAL_FRAME_HEADER_LEN: u64 = 24;
/// WAL magic number; the low bit selects the checksum byte order.
const WAL_MAGIC: u32 = 0x377f_0682;
const MIN_PAGE_SIZE: u32 = 512;
const MAX_PAGE_SIZE: u32 = 65_536;

/// Why [`check`] refused a database.
#[derive(Debug)]
pub(crate) enum WalSidecarError {
    /// The sidecar would make `SQLite` read pages at the wrong size.
    Mismatch(String),
    /// The database or its sidecar could not be read.
    Io(std::io::Error),
}

impl From<std::io::Error> for WalSidecarError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Returns the path of the `-wal` sidecar `SQLite` uses for `db_path`.
pub(crate) fn wal_path(db_path: &Path) -> PathBuf {
    let mut path = db_path.as_os_str().to_os_string();
    path.push("-wal");
    PathBuf::from(path)
}

/// Checks that the `-wal` sidecar of `db_path`, if `SQLite` would replay it,
/// uses the same page size as the database.
pub(crate) fn check(db_path: &Path) -> Result<(), WalSidecarError> {
    let wal_path = wal_path(db_path);
    let Some(mut wal) = open_existing(&wal_path)? else {
        return Ok(());
    };
    let mut wal_header = [0_u8; WAL_HEADER_LEN];
    if !read_prefix(&mut wal, &mut wal_header)? {
        return Ok(());
    }
    let Some(wal_page_size) = wal_header_page_size(&wal_header) else {
        return Ok(());
    };

    let Some(mut db) = open_existing(db_path)? else {
        return Ok(());
    };
    if db.metadata()?.len() == 0 {
        return Ok(());
    }
    let mut db_header = [0_u8; DB_HEADER_LEN];
    let db_page_size = if read_prefix(&mut db, &mut db_header)? {
        db_header_page_size(&db_header)
    } else {
        None
    };
    match db_page_size {
        None => {
            return Err(mismatch(
                db_path,
                &wal_path,
                &format!(
                    "the WAL holds {wal_page_size}-byte pages but the database header \
                     has no valid page size"
                ),
            ));
        }
        Some(size) if size != wal_page_size => {
            return Err(mismatch(
                db_path,
                &wal_path,
                &format!(
                    "the WAL holds {wal_page_size}-byte pages but the database uses \
                     {size}-byte pages"
                ),
            ));
        }
        Some(_) => {}
    }
    check_page_one_frames(&mut wal, wal_page_size).map_err(|error| match error {
        WalSidecarError::Mismatch(detail) => mismatch(db_path, &wal_path, &detail),
        io @ WalSidecarError::Io(_) => io,
    })
}

/// Checks every complete WAL frame that holds page 1. `SQLite` adopts the
/// page size recorded in the newest page 1, so a copy that records another
/// size would change the pager's page size after the header check.
fn check_page_one_frames(wal: &mut File, page_size: u32) -> Result<(), WalSidecarError> {
    let wal_len = wal.metadata()?.len();
    let frame_len = WAL_FRAME_HEADER_LEN + u64::from(page_size);
    let mut offset = WAL_HEADER_LEN as u64;
    let mut page_number = [0_u8; 4];
    let mut page_header = [0_u8; DB_HEADER_LEN];
    // A trailing partial frame is never replayed, so only complete frames
    // are inspected.
    while offset
        .checked_add(frame_len)
        .is_some_and(|end| end <= wal_len)
    {
        wal.seek(SeekFrom::Start(offset))?;
        wal.read_exact(&mut page_number)?;
        if u32::from_be_bytes(page_number) == 1 {
            wal.seek(SeekFrom::Start(offset + WAL_FRAME_HEADER_LEN))?;
            wal.read_exact(&mut page_header)?;
            // Page 1 without the magic is rejected by SQLite before it reads
            // the page size, so only a well-formed header can change it.
            if page_header.starts_with(DB_MAGIC)
                && db_header_page_size(&page_header) != Some(page_size)
            {
                return Err(WalSidecarError::Mismatch(format!(
                    "the WAL holds {page_size}-byte pages but a copy of page 1 at byte \
                     {offset} records a different page size"
                )));
            }
        }
        offset += frame_len;
    }
    Ok(())
}

fn mismatch(db_path: &Path, wal_path: &Path, detail: &str) -> WalSidecarError {
    WalSidecarError::Mismatch(format!(
        "refusing to open {}: {detail} (CVE-2026-119375). Legitimate WAL files always \
         match the database page size, so the files are corrupt or crafted. Delete {} \
         and its -shm sidecar, or the whole database, and re-index.",
        db_path.display(),
        wal_path.display(),
    ))
}

/// Opens `path` for reading, or returns `None` if it does not exist.
fn open_existing(path: &Path) -> std::io::Result<Option<File>> {
    match File::open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Fills `buf` from the start of `file`. Returns `false` if the file is
/// shorter than `buf`.
fn read_prefix(file: &mut File, buf: &mut [u8]) -> std::io::Result<bool> {
    match file.read_exact(buf) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

fn is_valid_page_size(size: u32) -> bool {
    size.is_power_of_two() && (MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&size)
}

/// Decodes the page size from a database header, as `SQLite`'s `lockBtree`
/// does: bytes 16..18 big-endian, where the value 1 means 65536.
fn db_header_page_size(header: &[u8]) -> Option<u32> {
    if !header.starts_with(DB_MAGIC) {
        return None;
    }
    let high = u32::from(*header.get(16)?);
    let low = u32::from(*header.get(17)?);
    let size = (high << 8) | (low << 16);
    is_valid_page_size(size).then_some(size)
}

/// Returns the page size from a WAL header, or `None` if `SQLite` would
/// ignore the WAL because the magic number or page size is invalid.
fn wal_header_page_size(header: &[u8; WAL_HEADER_LEN]) -> Option<u32> {
    let magic = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    if magic & !1 != WAL_MAGIC {
        return None;
    }
    let size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    is_valid_page_size(size).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_header(high: u8, low: u8) -> [u8; DB_HEADER_LEN] {
        let mut header = [0_u8; DB_HEADER_LEN];
        header[..16].copy_from_slice(DB_MAGIC);
        header[16] = high;
        header[17] = low;
        header
    }

    fn wal_header(magic: u32, page_size: u32) -> [u8; WAL_HEADER_LEN] {
        let mut header = [0_u8; WAL_HEADER_LEN];
        header[..4].copy_from_slice(&magic.to_be_bytes());
        header[8..12].copy_from_slice(&page_size.to_be_bytes());
        header
    }

    #[test]
    fn decodes_database_page_sizes_like_sqlite() {
        assert_eq!(db_header_page_size(&db_header(0x02, 0x00)), Some(512));
        assert_eq!(db_header_page_size(&db_header(0x10, 0x00)), Some(4096));
        assert_eq!(db_header_page_size(&db_header(0x80, 0x00)), Some(32_768));
        assert_eq!(db_header_page_size(&db_header(0x00, 0x01)), Some(65_536));
        assert_eq!(db_header_page_size(&db_header(0x01, 0x00)), None);
        assert_eq!(db_header_page_size(&db_header(0x10, 0x01)), None);
        let mut not_sqlite = db_header(0x10, 0x00);
        not_sqlite[0] = b'X';
        assert_eq!(db_header_page_size(&not_sqlite), None);
        assert_eq!(db_header_page_size(DB_MAGIC), None);
    }

    #[test]
    fn decodes_wal_headers_sqlite_would_replay() {
        assert_eq!(
            wal_header_page_size(&wal_header(0x377f_0682, 4096)),
            Some(4096)
        );
        assert_eq!(
            wal_header_page_size(&wal_header(0x377f_0683, 65_536)),
            Some(65_536)
        );
        assert_eq!(wal_header_page_size(&wal_header(0x377f_0684, 4096)), None);
        assert_eq!(wal_header_page_size(&wal_header(0x377f_0682, 4095)), None);
        assert_eq!(wal_header_page_size(&wal_header(0x377f_0682, 256)), None);
        assert_eq!(
            wal_header_page_size(&wal_header(0x377f_0682, 131_072)),
            None
        );
    }

    #[test]
    fn wal_path_appends_suffix_to_file_name() {
        assert_eq!(
            wal_path(Path::new("index.db")),
            PathBuf::from("index.db-wal")
        );
    }
}
