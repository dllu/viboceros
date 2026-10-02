//! Application preferences; command starts and successful completions are distinct.
use super::*;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) struct CopyPreferences(Arc<Mutex<State>>);

struct State {
    enabled: bool,
    values: BTreeMap<&'static str, bool>,
}

impl Default for CopyPreferences {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(State {
            enabled: true,
            values: BTreeMap::new(),
        })))
    }
}

impl CopyPreferences {
    pub(super) fn enabled(&self) -> bool {
        self.0.lock().expect("preference lock").enabled
    }

    pub(super) fn set_enabled(&self, value: bool) {
        self.0.lock().expect("preference lock").enabled = value;
    }

    pub(super) fn peek(&self, name: &'static str, default: bool) -> bool {
        let state = self.0.lock().expect("preference lock");
        if state.enabled {
            state.values.get(name).copied().unwrap_or(default)
        } else {
            default
        }
    }

    pub(super) fn begin(&self, name: &'static str, default: bool) -> bool {
        let mut state = self.0.lock().expect("preference lock");
        let enabled = state.enabled;
        let value = state.values.entry(name).or_insert(default);
        if !enabled {
            *value = default;
        }
        *value
    }

    pub(super) fn complete(&self, name: &'static str, value: bool) {
        self.0
            .lock()
            .expect("preference lock")
            .values
            .insert(name, value);
    }
}

/// Supply an omitted Copy argument without changing explicit or invalid input.
/// The returned value can be remembered only after successful execution.
pub(super) fn arguments<'a>(input: &[&'a str], default: bool) -> (Vec<&'a str>, Option<bool>) {
    let mut copy = None;
    let mut seen = false;
    let mut invalid = false;
    let mut index = 0;
    while index < input.len() {
        let token = input[index];
        let (name, value) = if let Some((name, value)) = token.split_once('=') {
            (name, Some(value))
        } else if option_name_eq(token, "Copy") {
            index += 1;
            (token, input.get(index).copied())
        } else if option_name_eq(token, "SurfaceName") {
            // OrientOnSrf accepts bare name/value pairs. A surface may itself
            // be named Copy; its name is not another command option.
            index += 2;
            continue;
        } else {
            index += 1;
            continue;
        };
        if option_name_eq(name, "Copy") {
            invalid |= seen;
            seen = true;
            copy = value.and_then(parse_yes_no);
            invalid |= copy.is_none();
        }
        index += 1;
    }
    let mut result = input.to_vec();
    if !seen {
        result.push(if default { "Copy=Yes" } else { "Copy=No" });
        copy = Some(default);
    }
    (result, if invalid { None } else { copy })
}

pub(super) struct RememberCopyOptionsCommand(pub(super) CopyPreferences);

impl Command for RememberCopyOptionsCommand {
    fn name(&self) -> &'static str {
        "RememberCopyOptions"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, _document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        match arguments {
            [] => {}
            [choice] => self.0.set_enabled(
                parse_yes_no(choice).ok_or(CommandError::Usage("RememberCopyOptions [Yes|No]"))?,
            ),
            _ => return Err(CommandError::Usage("RememberCopyOptions [Yes|No]")),
        }
        Ok(format!(
            "Remember copy options: {}",
            if self.0.enabled() { "Yes" } else { "No" }
        ))
    }
}
