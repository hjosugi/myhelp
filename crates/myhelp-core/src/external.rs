use crate::{Error, Result, is_symlink_or_reparse, open_read_nofollow, reject_oversized_page};
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;

#[derive(Clone, Copy)]
pub(crate) enum ExternalFileKind {
    Page,
    Input(&'static str),
}

impl ExternalFileKind {
    fn label(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Input(field) => field,
        }
    }
}

pub(crate) fn read_external_utf8(
    path: &Path,
    max_bytes: usize,
    kind: ExternalFileKind,
) -> Result<String> {
    let mut file = checked_external_file(path, max_bytes, kind)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    reject_external_size(path, bytes.len() as u64, max_bytes, kind)?;
    String::from_utf8(bytes).map_err(|error| {
        Error::Io(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("{} is not valid UTF-8: {error}", kind.label()),
        ))
    })
}

pub(crate) fn read_external_bytes(
    path: &Path,
    max_bytes: usize,
    kind: ExternalFileKind,
) -> Result<Vec<u8>> {
    let mut file = checked_external_file(path, max_bytes, kind)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    reject_external_size(path, bytes.len() as u64, max_bytes, kind)?;
    Ok(bytes)
}

fn checked_external_file(path: &Path, max_bytes: usize, kind: ExternalFileKind) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_external_size(path, metadata.len(), max_bytes, kind)?;

    let file = open_read_nofollow(path)?;
    let metadata = file.metadata()?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_external_size(path, metadata.len(), max_bytes, kind)?;
    Ok(file)
}

fn reject_external_size(
    path: &Path,
    size: u64,
    max_bytes: usize,
    kind: ExternalFileKind,
) -> Result<()> {
    if size <= max_bytes as u64 {
        return Ok(());
    }

    match kind {
        ExternalFileKind::Page => reject_oversized_page(path, size),
        ExternalFileKind::Input(field) => Err(Error::InputTooLarge { field, max_bytes }),
    }
}
