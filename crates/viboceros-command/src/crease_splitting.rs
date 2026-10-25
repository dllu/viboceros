//! Application preferences shared by surface construction commands, outside undo.
use super::*;
use std::sync::Arc;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct Setting(pub bool);
impl Default for Setting {
    fn default() -> Self {
        Self(true)
    }
}
pub(super) type Preference = Arc<remembered::Remembered<Setting>>;
pub(super) struct CreaseSplittingCommand(pub Preference);

impl Command for CreaseSplittingCommand {
    fn name(&self) -> &'static str {
        "CreaseSplitting"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, _doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        const USAGE: &str = "CreaseSplitting Enable|Disable|Toggle";
        let [action] = args else {
            return Err(CommandError::Usage(USAGE));
        };
        let enabled = if option_name_eq(action, "Enable") {
            true
        } else if option_name_eq(action, "Disable") {
            false
        } else if option_name_eq(action, "Toggle") {
            !self.0.get().0
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        self.0.set(Setting(enabled));
        Ok(format!(
            "Crease splitting {}",
            if enabled { "enabled" } else { "disabled" }
        ))
    }
}
