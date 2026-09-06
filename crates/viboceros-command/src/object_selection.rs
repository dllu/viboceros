//! Command-owned object filters and boolean options for selection prompts.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectSelectionFilter {
    #[default]
    Any,
    Mesh,
}

impl ObjectSelectionFilter {
    pub fn accepts(self, geometry: &Geometry) -> bool {
        match self {
            Self::Any => true,
            Self::Mesh => matches!(geometry, Geometry::Mesh(_)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooleanSelectionOption {
    pub name: &'static str,
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectSelectionPrompt {
    pub command: &'static str,
    pub filter: ObjectSelectionFilter,
    pub options: Vec<BooleanSelectionOption>,
}

impl ObjectSelectionPrompt {
    /// Stage an entire input before accepting any option, including duplicates.
    pub fn update_options(&mut self, input: &str) -> Result<(), CommandError> {
        const USAGE: &str = "known-option=Yes|No [known-option=Yes|No ...]";
        let arguments = input.split_whitespace().collect::<Vec<_>>();
        if arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut options = self.options.clone();
        let mut seen = BTreeSet::new();
        let mut i = 0;
        while i < arguments.len() {
            let (name, value, consumed) = orient_option(&arguments, i, USAGE)?;
            let option = options
                .iter_mut()
                .find(|o| option_name_eq(name, o.name))
                .ok_or(CommandError::Usage(USAGE))?;
            if !seen.insert(option.name) {
                return Err(CommandError::Usage(USAGE));
            }
            option.value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            i += consumed;
        }
        self.options = options;
        Ok(())
    }

    pub fn command_line(&self) -> String {
        let mut input = self.command.to_owned();
        for option in &self.options {
            input.push_str(&format!(
                " {}={}",
                option.name,
                if option.value { "Yes" } else { "No" }
            ));
        }
        input
    }
}

impl CommandRegistry {
    pub fn accept_object_selection_options(
        &self,
        prompt: &ObjectSelectionPrompt,
    ) -> Result<(), CommandError> {
        let input = prompt.command_line();
        let mut tokens = input.split_whitespace();
        let name = normalize_command_name(tokens.next().ok_or(CommandError::EmptyInput)?);
        let index = self
            .lookup
            .get(&name)
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        self.commands[*index].accept_object_selection_options(&tokens.collect::<Vec<_>>())
    }

    /// Reads a command's current choices without editing the document or memory.
    pub fn object_selection_prompt(
        &self,
        input: &str,
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let mut arguments = input.split_whitespace();
        let Some(name) = arguments.next() else {
            return Ok(None);
        };
        let Some(index) = self.lookup.get(&normalize_command_name(name)) else {
            return Ok(None);
        };
        self.commands[*index].object_selection_prompt(&arguments.collect::<Vec<_>>())
    }
}
