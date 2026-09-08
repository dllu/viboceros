use super::*;

/// A named action that swaps two values. Other choices make it unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionToggle {
    pub name: &'static str,
    pub values: [&'static str; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceSelectionOption {
    pub name: &'static str,
    pub value: &'static str,
    pub choices: &'static [&'static str],
    pub toggle: Option<SelectionToggle>,
}

impl ChoiceSelectionOption {
    pub fn set(&mut self, input: &str) -> Result<(), CommandError> {
        self.value = self
            .choices
            .iter()
            .copied()
            .find(|value| option_name_eq(input.trim(), value))
            .ok_or(CommandError::Usage("one of the displayed option values"))?;
        Ok(())
    }

    pub(super) fn toggle(&mut self) -> Result<(), CommandError> {
        let toggle = self
            .toggle
            .as_ref()
            .ok_or(CommandError::Usage("available option action"))?;
        let next = if self.value == toggle.values[0] {
            toggle.values[1]
        } else if self.value == toggle.values[1] {
            toggle.values[0]
        } else {
            return Err(CommandError::Usage(
                "action is unavailable for the current choice",
            ));
        };
        self.set(next)
    }
}
