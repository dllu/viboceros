//! Same-directory staging shared by all path-based CAD exporters.
use std::{fs::File, io, path::Path};

pub(crate) struct StagedFile<'a> {
    destination: &'a Path,
    temporary: tempfile::NamedTempFile,
}

impl<'a> StagedFile<'a> {
    pub(crate) fn new(destination: &'a Path, suffix: &str) -> io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let temporary = tempfile::Builder::new()
            .prefix(".viboceros-")
            .suffix(suffix)
            .tempfile_in(parent)?;
        Ok(Self {
            destination,
            temporary,
        })
    }

    pub(crate) fn file(&self) -> &File {
        self.temporary.as_file()
    }
    pub(crate) fn path(&self) -> &Path {
        self.temporary.path()
    }

    /// Call only after format writers have completed and flushed their buffers.
    /// Synchronize the staged contents before replacing the directory entry.
    /// This is not a directory-fsync/power-loss durability guarantee.
    pub(crate) fn commit(self) -> io::Result<()> {
        self.file().sync_all()?;
        self.temporary
            .persist(self.destination)
            .map_err(|error| error.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};

    #[test]
    fn abandoned_partial_write_preserves_destination_and_removes_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("model.stl");
        fs::write(&destination, b"original").unwrap();
        fn write_then_fail(staged: StagedFile<'_>) -> io::Result<()> {
            staged.file().write_all(b"partial")?;
            Err(io::Error::other("injected writer failure"))
        }
        let staged = StagedFile::new(&destination, ".stl.tmp").unwrap();
        let temporary = staged.path().to_owned();
        let result = write_then_fail(staged);
        assert!(result.is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        assert!(!temporary.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn successful_commit_replaces_only_the_destination() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("model.step");
        fs::write(&destination, b"original").unwrap();
        let staged = StagedFile::new(&destination, ".step.tmp").unwrap();
        let temporary = staged.path().to_owned();
        assert_eq!(temporary.parent(), destination.parent());
        staged.file().write_all(b"complete replacement").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        staged.commit().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"complete replacement");
        assert!(!temporary.exists());
    }

    #[test]
    fn failed_commit_preserves_directory_and_cleans_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("model.3dm");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), b"original").unwrap();
        let staged = StagedFile::new(&destination, ".3dm.tmp").unwrap();
        let temporary = staged.path().to_owned();
        staged.file().write_all(b"replacement").unwrap();
        assert!(staged.commit().is_err());
        assert_eq!(fs::read(destination.join("keep")).unwrap(), b"original");
        assert!(!temporary.exists());
    }
}
