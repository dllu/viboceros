//! Count and option prompts, shared with the scripted option parser.
use super::*;
use viboceros_command::CommandError;
fn option_name_eq(actual: &str, expected: &str) -> bool {
    actual
        .trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case(expected)
}
use viboceros_geometry::MAX_POINT_GRID_AXIS_COUNT;

impl Session {
    pub(super) fn parameters(&mut self, words: &[&str]) -> Result<usize, CommandError> {
        let mut index = 0;
        while index < words.len() {
            let word = words[index];
            let pending = self.pending;
            let combined = pending.map(|name| format!("{name}={word}"));
            let one = [combined.as_deref().unwrap_or_default()];
            let args = if pending.is_some() {
                &one[..]
            } else {
                &words[index..]
            };
            let bare_degree =
                if self.control && self.count.len() < 2 && option_name_eq(word, "Degree") {
                    Some("Degree")
                } else if !self.control && self.count.len() < 2 {
                    ["DegreeU", "DegreeV"]
                        .into_iter()
                        .find(|n| option_name_eq(word, n))
                } else {
                    None
                };
            if pending.is_none() && bare_degree.is_some() && index + 1 == words.len() {
                self.pending = bare_degree;
                return Ok(words.len());
            }
            let mut options = self.options;
            if let Some(n) = options.parse(args, self.control, self.count.len())? {
                if self.count.iter().enumerate().any(|(axis, &count)| {
                    count <= options.degree[axis] || options.closed[axis] && count < 3
                }) {
                    return Err(CommandError::Usage(
                        "grid counts must exceed their degrees; closed directions require three points",
                    ));
                }
                self.options = options;
                self.preferences.degree = options.degree;
                self.pending = None;
                index += if pending.is_some() { 1 } else { n };
            } else if self.count.len() < 2 {
                let axis = self.count.len();
                let count = solver::integer(word)
                    .filter(|n| {
                        (2..=MAX_POINT_GRID_AXIS_COUNT).contains(n)
                            && *n > self.options.degree[axis]
                            && (!self.options.closed[axis] || *n >= 3)
                    })
                    .ok_or(CommandError::Usage(
                        "grid count must exceed its degree and fit within 256 points",
                    ))?;
                self.preferences.counts[axis] = count;
                self.count.push(count);
                index += 1;
            } else {
                break;
            }
        }
        Ok(index)
    }
}
