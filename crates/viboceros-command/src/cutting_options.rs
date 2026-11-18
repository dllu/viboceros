//! Output options shared by the plane cutting commands.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    #[default]
    CurvesOnly,
    All,
}

impl Output {
    pub fn label(self) -> &'static str {
        match self {
            Self::CurvesOnly => "CurvesOnly",
            Self::All => "All",
        }
    }
}

pub(crate) fn properties_label(input: bool) -> &'static str {
    if input {
        "ByInputObject"
    } else {
        "ByCurrentLayer"
    }
}

pub(crate) fn update(
    input_properties: &mut bool,
    output: &mut Output,
    key: &str,
    value: Option<&str>,
    usage: &'static str,
) -> Result<bool, CommandError> {
    if option_name_eq(key, "AssignProperties") {
        *input_properties = match value {
            None => !*input_properties,
            Some(v) if option_name_eq(v, "ByInputObject") => true,
            Some(v) if option_name_eq(v, "ByCurrentLayer") => false,
            _ => return Err(CommandError::Usage(usage)),
        };
    } else if option_name_eq(key, "Output") {
        *output = match value {
            Some(v) if option_name_eq(v, "CurvesOnly") => Output::CurvesOnly,
            Some(v) if option_name_eq(v, "All") => Output::All,
            None if *output == Output::CurvesOnly => Output::All,
            None => Output::CurvesOnly,
            _ => return Err(CommandError::Usage(usage)),
        };
    } else {
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn toggle(
    current: bool,
    value: Option<&str>,
    usage: &'static str,
) -> Result<bool, CommandError> {
    match value {
        None => Ok(!current),
        Some(v) if option_name_eq(v, "Yes") => Ok(true),
        Some(v) if option_name_eq(v, "No") => Ok(false),
        _ => Err(CommandError::Usage(usage)),
    }
}

// Complete command argument blocks retain their original duplicate checks.
pub(crate) fn apply_words<T>(
    mut options: T,
    words: &[&str],
    usage: &'static str,
    update: impl Fn(&mut T, &str) -> Result<bool, CommandError>,
) -> Result<T, CommandError> {
    let mut seen = BTreeSet::new();
    for &word in words {
        let key = word
            .split('=')
            .next()
            .unwrap()
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase();
        if !seen.insert(key) || !update(&mut options, word)? {
            return Err(CommandError::Usage(usage));
        }
    }
    Ok(options)
}

pub(crate) fn choices(input: bool, output: Output) -> Vec<ChoiceSelectionOption> {
    [
        (
            "AssignProperties",
            properties_label(input),
            &["ByCurrentLayer", "ByInputObject"] as &[_],
        ),
        ("Output", output.label(), &["CurvesOnly", "All"] as &[_]),
    ]
    .into_iter()
    .map(|(name, value, choices)| ChoiceSelectionOption {
        name,
        value,
        choices,
        toggle: Some(SelectionToggle {
            name,
            values: [choices[0], choices[1]],
        }),
    })
    .collect()
}

pub(crate) fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}
