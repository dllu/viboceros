use std::path::PathBuf;
use viboceros_command::CommandRegistry;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Completion {
    pub label: String,
    pub replacement: String,
    pub directory: bool,
}

#[derive(Default)]
pub(super) struct CompletionState {
    query: String,
    pub candidates: Vec<Completion>,
    pub selected: Option<usize>,
    applied: Option<String>,
}

impl CompletionState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn refresh(&mut self, commands: &CommandRegistry, input: &str, enabled: bool) {
        if !enabled {
            self.reset();
            return;
        }
        if input == self.query || self.applied.as_deref() == Some(input) {
            return;
        }
        self.query = input.to_owned();
        self.candidates = completions(commands, input);
        self.selected = None;
        self.applied = None;
    }

    pub fn cycle(&mut self, reverse: bool) -> Option<String> {
        let count = self.candidates.len();
        if count == 0 {
            return None;
        }
        let index = match self.selected {
            Some(index) if reverse => (index + count - 1) % count,
            Some(index) => (index + 1) % count,
            None if reverse => count - 1,
            None => 0,
        };
        self.choose(index)
    }

    pub fn choose(&mut self, index: usize) -> Option<String> {
        let candidate = self.candidates.get(index)?;
        let replacement = candidate.replacement.clone();
        let directory = candidate.directory;
        self.selected = Some(index);
        self.applied = Some(replacement.clone());
        // Directory completion descends on the next Tab. Files and commands
        // continue cycling the original candidate set until the user edits.
        if directory {
            self.reset();
        }
        Some(replacement)
    }
}

pub(crate) fn command_completions(commands: &CommandRegistry, input: &str) -> Vec<&'static str> {
    let input = input.trim_start();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let query = input
        .trim_start_matches(['\'', '_', '-'])
        .to_ascii_lowercase();
    if query.is_empty() || query.len() > 128 {
        return Vec::new();
    }
    let mut names = commands
        .command_names()
        .into_iter()
        .chain(viboceros_command::interface::COMMAND_NAMES)
        .chain([
            "CPlane",
            "NamedView",
            "ReadViewportsFromFile",
            "SetActiveViewport",
            "SetMaximizedViewport",
            "ViewportProperties",
            "Help",
        ])
        .filter_map(|name| score(&query, &name.to_ascii_lowercase()).map(|score| (score, name)))
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup_by_key(|entry| entry.1);
    names.into_iter().map(|(_, name)| name).collect()
}

fn score(query: &str, candidate: &str) -> Option<(usize, usize)> {
    if candidate.starts_with(query) {
        return Some((0, 0));
    }
    if let Some(start) = candidate.find(query) {
        return Some((1, start));
    }
    let mut remaining = candidate;
    let mut skipped = 0;
    let subsequence = query.chars().all(|character| {
        if let Some(index) = remaining.find(character) {
            skipped += index;
            remaining = &remaining[index + character.len_utf8()..];
            true
        } else {
            false
        }
    });
    if subsequence {
        return Some((2, skipped));
    }
    // Short inputs would produce too many unrelated typo matches.
    let limit = match query.len() {
        0..=3 => return None,
        4..=5 => 1,
        _ => 2,
    };
    let distance = edit_distance(query, candidate);
    (distance <= limit).then_some((3, distance))
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<_> = b.chars().collect();
    let mut previous: Vec<_> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, a) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, b) in b.iter().enumerate() {
            current[j + 1] = (current[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + usize::from(a != *b));
        }
        std::mem::swap(&mut current, &mut previous);
    }
    previous[b.len()]
}

fn completions(commands: &CommandRegistry, input: &str) -> Vec<Completion> {
    if let Some((prefix, path)) = path_argument(input) {
        return path_completions(prefix, path);
    }
    let trimmed = input.trim_start();
    let name = trimmed.trim_start_matches(['\'', '_', '-']);
    let prefix = &input[..input.len() - name.len()];
    command_completions(commands, input)
        .into_iter()
        .map(|name| Completion {
            label: name.to_owned(),
            replacement: format!("{prefix}{name} "),
            directory: false,
        })
        .collect()
}

// The file commands consume the entire remaining string as one path. Only
// ExportStl's format and ImportStep's native flag precede it; mirror that grammar.
fn path_argument(input: &str) -> Option<(&str, &str)> {
    let start = input.len() - input.trim_start().len();
    let end = start + input[start..].find(char::is_whitespace)?;
    let command = input[start..end]
        .trim_start_matches(['\'', '_', '-'])
        .to_ascii_lowercase();
    if !matches!(
        command.as_str(),
        "readviewportsfromfile"
            | "import3dm"
            | "export3dm"
            | "importstl"
            | "exportstl"
            | "importstep"
            | "exportstep"
            | "importstp"
            | "exportstp"
    ) {
        return None;
    }
    let mut offset = input.len() - input[end..].trim_start().len();
    let rest = &input[offset..];
    let token_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let first = &rest[..token_end];
    let option = command == "exportstl"
        && (first.eq_ignore_ascii_case("ascii") || first.eq_ignore_ascii_case("binary"))
        || matches!(command.as_str(), "importstep" | "importstp")
            && first.eq_ignore_ascii_case("Native=Yes");
    if option {
        // A complete option without a space is still being edited.
        if token_end == rest.len() {
            return Some((input, ""));
        }
        offset += token_end;
        offset = input.len() - input[offset..].trim_start().len();
    }
    Some((&input[..offset], &input[offset..]))
}

fn path_completions(prefix: &str, path: &str) -> Vec<Completion> {
    let quoted = path.starts_with('"');
    let path = if quoted {
        path[1..].strip_suffix('"').unwrap_or(&path[1..])
    } else {
        path
    };
    if path.contains('"') || path.chars().any(char::is_control) {
        return Vec::new();
    }
    let expanded;
    let path = if path == "~" || path.starts_with("~/") || cfg!(windows) && path.starts_with("~\\")
    {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
        let Some(home) = home.and_then(|h| h.into_string().ok()) else {
            return Vec::new();
        };
        expanded = format!(
            "{}{separator}{}",
            home.trim_end_matches(std::path::MAIN_SEPARATOR),
            path.get(2..).unwrap_or(""),
            separator = std::path::MAIN_SEPARATOR
        );
        &expanded
    } else {
        path
    };
    let split = path
        .rfind(|c| c == '/' || cfg!(windows) && c == '\\')
        .map_or(0, |i| i + 1);
    let (parent, partial) = path.split_at(split);
    let directory = if parent.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(parent)
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let query = partial.to_lowercase();
    let separator = if parent.ends_with('\\') { '\\' } else { '/' };
    let prefix = if prefix.ends_with(char::is_whitespace) {
        prefix.to_owned()
    } else {
        format!("{prefix} ")
    };
    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.to_lowercase().starts_with(&query)
            || name.contains('"')
            || name.chars().any(char::is_control)
            || name.starts_with('.') && !partial.starts_with('.')
        {
            continue;
        }
        let is_directory = entry.path().is_dir();
        let full = format!(
            "{parent}{name}{}",
            if is_directory {
                separator.to_string()
            } else {
                String::new()
            }
        );
        // Quote files even without spaces: names such as Binary or Native=Yes
        // must not be reinterpreted as an option by the command parser.
        let replacement = format!("{prefix}\"{full}\"");
        matches.push(Completion {
            label: format!(
                "{name}{}",
                if is_directory {
                    separator.to_string()
                } else {
                    String::new()
                }
            ),
            replacement,
            directory: is_directory,
        });
    }
    matches.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
            .then_with(|| a.label.cmp(&b.label))
    });
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_ranking_and_tab_cycle_preserve_the_original_query() {
        let commands = CommandRegistry::with_builtins();
        assert_eq!(
            command_completions(&commands, "readviewport")[0],
            "ReadViewportsFromFile"
        );
        assert_eq!(
            command_completions(&commands, "setactiveviewport")[0],
            "SetActiveViewport"
        );
        assert_eq!(command_completions(&commands, "mshsph")[0], "MeshSphere");
        assert_eq!(command_completions(&commands, "circel")[0], "Circle");
        assert_eq!(
            command_completions(&commands, "_pOlY")[..2],
            ["Polygon", "Polyline"]
        );
        let mut state = CompletionState::default();
        state.refresh(&commands, "po", true);
        assert_eq!(state.cycle(false).as_deref(), Some("Point "));
        state.refresh(&commands, "Point ", true);
        assert_eq!(state.cycle(false).as_deref(), Some("PointCloud "));
        assert_eq!(state.cycle(true).as_deref(), Some("Point "));
        state.refresh(&commands, "Point 1,2,3", true);
        assert!(state.candidates.is_empty());
    }

    #[test]
    fn paths_preserve_options_spaces_unicode_and_directory_descent() {
        let directory = super::super::tests::TempDirectory::new();
        std::fs::create_dir(directory.0.join("parts with spaces")).unwrap();
        std::fs::write(directory.0.join("parts with spaces/模型.3dm"), "").unwrap();
        let commands = CommandRegistry::with_builtins();
        let input = format!("_Import3dm {}/par", directory.0.display());
        let candidates = completions(&commands, &input);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].directory);
        assert!(candidates[0].replacement.ends_with("parts with spaces/\""));
        let files = completions(&commands, &candidates[0].replacement);
        assert_eq!(files.len(), 1);
        assert!(files[0].replacement.ends_with("模型.3dm\""));
        for command in [
            "ReadViewportsFromFile",
            "ExportStl Binary",
            "ExportStl Ascii",
            "ImportStep Native=Yes",
            "ImportStp Native=Yes",
        ] {
            let input = format!("{command} \"{}/parts with spaces/模", directory.0.display());
            assert!(
                completions(&commands, &input)[0]
                    .replacement
                    .starts_with(&format!("{command} \""))
            );
        }
        assert!(
            completions(
                &commands,
                &format!("Import3dm {}/missing/x", directory.0.display())
            )
            .is_empty()
        );
        assert!(path_argument("Move 0,0,0 1,1,1").is_none());
    }
}
