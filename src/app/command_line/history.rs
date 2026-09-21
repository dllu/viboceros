//! Bounded recall with an append-only, per-user JSON-lines journal.
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

const MAX_ENTRIES: usize = 1000;
const MAX_ENTRY_BYTES: usize = 16 * 1024;
const LOAD_BYTES: u64 = 1024 * 1024;

#[derive(Default)]
pub(super) struct History {
    entries: VecDeque<String>,
    path: Option<PathBuf>,
    cursor: Option<usize>,
    draft: String,
    recalled: Option<String>,
}

impl History {
    pub(super) fn load(path: Option<PathBuf>) -> (Self, Option<String>) {
        let mut history = Self {
            path,
            ..Self::default()
        };
        let error = history
            .read()
            .err()
            .map(|error| format!("Could not read command history: {error}"));
        (history, error)
    }

    fn read(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        file.lock_shared()?;
        let size = file.metadata()?.len();
        let offset = size.saturating_sub(LOAD_BYTES);
        file.seek(SeekFrom::Start(offset))?;
        let mut reader = BufReader::new(file.take(LOAD_BYTES));
        let mut row = Vec::new();
        if offset > 0 {
            reader.read_until(b'\n', &mut row)?;
        }
        loop {
            row.clear();
            if reader.read_until(b'\n', &mut row)? == 0 {
                break;
            }
            // Skip a truncated record or malformed UTF-8/JSON without losing
            // the valid history after it. Tail seeking may begin inside UTF-8.
            if let Ok(entry) = serde_json::from_slice::<String>(&row) {
                self.push(&entry);
            }
        }
        Ok(())
    }

    fn push(&mut self, entry: &str) -> bool {
        if entry.trim().is_empty()
            || entry.len() > MAX_ENTRY_BYTES
            || self.entries.back().is_some_and(|last| last == entry)
        {
            return false;
        }
        if self.entries.len() == MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.to_owned());
        true
    }

    pub(super) fn remember(&mut self, entry: &str) -> io::Result<()> {
        self.reset_navigation();
        if !self.push(entry) {
            return Ok(());
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        let result = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut options = OpenOptions::new();
            options.create(true).append(true).read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(path)?;
            // Serialize append records from concurrent application sessions.
            file.lock()?;
            if file.metadata()?.len() > 0 {
                file.seek(SeekFrom::End(-1))?;
                let mut last = [0];
                file.read_exact(&mut last)?;
                if last[0] != b'\n' {
                    file.write_all(b"\n")?;
                }
            }
            let mut row = serde_json::to_vec(entry)?;
            row.push(b'\n');
            file.write_all(&row)
        })();
        if result.is_err() {
            self.path = None;
        }
        result
    }

    pub(super) fn reset_navigation(&mut self) {
        self.cursor = None;
        self.recalled = None;
        self.draft.clear();
    }

    pub(super) fn navigate(&mut self, input: &str, older: bool) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        // Editing a recalled command starts a new draft; Down can restore it.
        if self
            .recalled
            .as_deref()
            .is_some_and(|previous| previous != input)
        {
            self.reset_navigation();
        }
        let index = if older {
            match self.cursor {
                Some(index) => index.saturating_sub(1),
                None => {
                    self.draft = input.to_owned();
                    self.entries.len() - 1
                }
            }
        } else {
            let index = self.cursor? + 1;
            if index == self.entries.len() {
                let draft = self.draft.clone();
                self.reset_navigation();
                return Some(draft);
            }
            index
        };
        self.cursor = Some(index);
        let recalled = self.entries[index].clone();
        self.recalled = Some(recalled.clone());
        Some(recalled)
    }
}

pub(super) fn default_path() -> Option<PathBuf> {
    let nonempty = |name| {
        std::env::var_os(name)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(target_os = "windows") {
        nonempty("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        nonempty("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        nonempty("XDG_STATE_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| nonempty("HOME").map(|home| home.join(".local/state")))
    };
    base.map(|base| base.join("viboceros/command-history.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recall_restores_draft_and_preserves_edited_recall() {
        let mut h = History::default();
        for entry in ["Line", "Circle", "Circle"] {
            h.remember(entry).unwrap();
        }
        assert_eq!(h.navigate("unfinished", true).as_deref(), Some("Circle"));
        assert_eq!(h.navigate("Circle", true).as_deref(), Some("Line"));
        assert_eq!(h.navigate("Line", true).as_deref(), Some("Line"));
        assert_eq!(h.navigate("Line", false).as_deref(), Some("Circle"));
        assert_eq!(h.navigate("Circle", false).as_deref(), Some("unfinished"));
        assert_eq!(h.navigate("unfinished", false), None);
        assert_eq!(h.navigate("draft", true).as_deref(), Some("Circle"));
        assert_eq!(h.navigate("Circle 1,2,3", true).as_deref(), Some("Circle"));
        assert_eq!(h.navigate("Circle", false).as_deref(), Some("Circle 1,2,3"));
    }

    #[test]
    fn persisted_history_survives_sessions_unicode_and_malformed_rows() {
        let directory = super::super::tests::TempDirectory::new();
        let path = directory.0.join("nested/history.jsonl");
        let (mut a, error) = History::load(Some(path.clone()));
        assert!(error.is_none());
        a.remember("Import3dm \"a space/模型.3dm\"").unwrap();
        let (mut b, error) = History::load(Some(path.clone()));
        assert!(error.is_none());
        a.remember("Circle").unwrap();
        b.remember("Line").unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"not json\n\"unfinished")
            .unwrap();
        let (mut loaded, error) = History::load(Some(path));
        assert!(error.is_none());
        assert_eq!(loaded.navigate("", true).as_deref(), Some("Line"));
        assert_eq!(loaded.navigate("Line", true).as_deref(), Some("Circle"));
        assert_eq!(
            loaded.navigate("Circle", true).as_deref(),
            Some("Import3dm \"a space/模型.3dm\"")
        );
    }
    #[test]
    fn appending_after_truncated_record_recovers_and_save_errors_keep_recall() {
        let directory = super::super::tests::TempDirectory::new();
        let path = directory.0.join("history.jsonl");
        std::fs::write(&path, b"\"Line\"\n\"broken").unwrap();
        let (mut history, error) = History::load(Some(path.clone()));
        assert!(error.is_none());
        history.remember("Circle").unwrap();
        let (mut loaded, error) = History::load(Some(path));
        assert!(error.is_none());
        assert_eq!(loaded.navigate("", true).as_deref(), Some("Circle"));
        assert_eq!(loaded.navigate("Circle", true).as_deref(), Some("Line"));
        let blocker = directory.0.join("file-not-directory");
        std::fs::write(&blocker, "unchanged").unwrap();
        let (mut history, _) = History::load(Some(blocker.join("history.jsonl")));
        assert!(history.remember("Sphere").is_err());
        assert_eq!(history.navigate("", true).as_deref(), Some("Sphere"));
        assert!(history.remember("Box").is_ok()); // disabled persistence after reporting error
        assert_eq!(std::fs::read_to_string(blocker).unwrap(), "unchanged");
    }
}
