//! Numeric and option edits before accepting a Divide result.
use super::{CommandError, Options, Specification, USAGE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prompt {
    pub options: Options,
    count: usize,
    length: f64,
    chord: f64,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            options: Options {
                specification: Specification::Count(1),
                mark_ends: false,
                split: false,
                delete_remainder: false,
                group_output: false,
            },
            count: 1,
            length: 1.,
            chord: 1.,
        }
    }
}

impl Prompt {
    pub fn label(self) -> &'static str {
        match self.options.specification {
            Specification::Count(_) => "Number of segments",
            Specification::Length(_) => "Segment length",
            Specification::Chord(_) => "Chord length",
        }
    }

    pub fn command_line(self) -> String {
        let specification = match self.options.specification {
            Specification::Count(n) => n.to_string(),
            Specification::Length(n) => format!("Length {n}"),
            Specification::Chord(n) => format!("EqualChordLength {n}"),
        };
        let yes_no = |value| if value { "Yes" } else { "No" };
        format!(
            "Divide {specification} MarkEnds={} Split={} DeleteRemainder={} GroupOutput={}",
            yes_no(self.options.mark_ends),
            yes_no(self.options.split),
            yes_no(self.options.delete_remainder),
            yes_no(self.options.group_output),
        )
    }

    /// Stage every edit before returning a new prompt. Numeric answers update
    /// the pending result; the caller accepts only on a subsequent Enter.
    pub fn updated(self, input: &str) -> Result<Self, CommandError> {
        let mut staged = self;
        let mut positional = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for word in input.split_whitespace() {
            let word = word.trim_start_matches('_');
            let (name, value) = word
                .split_once('=')
                .map_or((word, None), |(k, v)| (k, Some(v)));
            let name = name.to_ascii_lowercase();
            let flag = match name.as_str() {
                "markends" => Some(&mut staged.options.mark_ends),
                "split" => Some(&mut staged.options.split),
                "deleteremainder" => Some(&mut staged.options.delete_remainder),
                "groupoutput" => Some(&mut staged.options.group_output),
                _ => None,
            };
            if let Some(flag) = flag {
                if !seen.insert(name) {
                    return Err(CommandError::Usage(USAGE));
                }
                *flag = match value.map(|v| v.trim_start_matches('_').to_ascii_lowercase()) {
                    None => !*flag,
                    Some(value) if value == "yes" => true,
                    Some(value) if value == "no" => false,
                    _ => return Err(CommandError::Usage(USAGE)),
                };
            } else {
                positional.push(word);
            }
        }
        let (mode, value) = match positional.as_slice() {
            [] => (None, None),
            [mode]
                if mode.eq_ignore_ascii_case("Length")
                    || mode.eq_ignore_ascii_case("EqualChordLength")
                    || mode.eq_ignore_ascii_case("NumberSegments") =>
            {
                (Some(*mode), None)
            }
            [value] => (None, Some(*value)),
            [mode, value] => (Some(*mode), Some(*value)),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let specification = match mode {
            Some(mode) if mode.eq_ignore_ascii_case("Length") => {
                Specification::Length(staged.length)
            }
            Some(mode) if mode.eq_ignore_ascii_case("EqualChordLength") => {
                Specification::Chord(staged.chord)
            }
            Some(mode) if mode.eq_ignore_ascii_case("NumberSegments") => {
                Specification::Count(staged.count)
            }
            Some(_) => return Err(CommandError::Usage(USAGE)),
            None => staged.options.specification,
        };
        staged.options.specification = if let Some(value) = value {
            let args = match specification {
                Specification::Count(_) => vec![value],
                Specification::Length(_) => vec!["Length", value],
                Specification::Chord(_) => vec!["EqualChordLength", value],
            };
            Options::parse(&args)?.specification
        } else {
            specification
        };
        match staged.options.specification {
            Specification::Count(n) => staged.count = n,
            Specification::Length(n) => staged.length = n,
            Specification::Chord(n) => staged.chord = n,
        }
        Ok(staged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_changes_retain_each_numeric_value_and_output_options() {
        let count = Prompt::default().updated("4 MarkEnds=Yes").unwrap();
        let length = count.updated("Length 2.5 Split=Yes").unwrap();
        let chord = length.updated("EqualChordLength 3").unwrap();
        let restored = chord.updated("Length").unwrap();
        assert_eq!(restored.options.specification, Specification::Length(2.5));
        assert!(restored.options.mark_ends && restored.options.split);
        assert_eq!(
            restored
                .updated("NumberSegments")
                .unwrap()
                .options
                .specification,
            Specification::Count(4)
        );
        assert_eq!(
            Options::parse(
                &restored
                    .command_line()
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            restored.options
        );
    }

    #[test]
    fn invalid_numbers_and_duplicate_options_do_not_change_the_prompt() {
        let prompt = Prompt::default().updated("Length 2.5").unwrap();
        for input in [
            "Split=Yes 0",
            "NaN",
            "-1",
            "Split=Yes Split=No",
            "Unknown=Yes",
            "Length 1 2",
        ] {
            assert!(prompt.updated(input).is_err(), "{input}");
            assert_eq!(prompt.options.specification, Specification::Length(2.5));
            assert!(!prompt.options.split);
        }
    }
}
