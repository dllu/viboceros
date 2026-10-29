//! A bounded UTF-8 report, including formatting of user-authored metadata.
use std::fmt;

pub(super) const MAX_REPORT_BYTES: usize = 256 * 1024;
const MARKER: &str = "\n[Report truncated; select fewer objects for more details]\n";

pub(super) struct Report {
    text: String,
    truncated: bool,
}
impl Report {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            truncated: false,
        }
    }
    pub fn is_full(&self) -> bool {
        self.truncated
    }
    pub fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str(MARKER);
        }
        self.text
    }
}
impl fmt::Write for Report {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let remaining = MAX_REPORT_BYTES - MARKER.len() - self.text.len();
        let mut end = value.len().min(remaining);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.truncated |= end < value.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    #[test]
    fn report_caps_multibyte_metadata_and_keeps_an_explicit_marker() {
        let mut text = Report::new();
        for _ in 0..MAX_REPORT_BYTES {
            write!(text, "🦏").unwrap();
        }
        let result = text.finish();
        assert!(result.len() <= MAX_REPORT_BYTES);
        assert!(result.ends_with(MARKER));
    }
}
