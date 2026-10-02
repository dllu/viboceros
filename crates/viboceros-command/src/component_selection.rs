//! Command-owned options for component picking, shared by GUI and typed input.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentSelectionKind {
    BrepEdge,
    BrepFace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberSelectionOption {
    pub name: &'static str,
    pub value: Real,
    pub minimum: Real,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentSelectionPrompt {
    pub command: &'static str,
    pub kind: ComponentSelectionKind,
    pub options: Vec<BooleanSelectionOption>,
    pub numbers: Vec<NumberSelectionOption>,
}

impl ComponentSelectionPrompt {
    pub fn command_line(&self) -> String {
        let mut input = self.command.to_owned();
        for option in &self.options {
            input.push_str(&format!(
                " {}={}",
                option.name,
                if option.value { "Yes" } else { "No" }
            ));
        }
        for option in &self.numbers {
            input.push_str(&format!(" {}={}", option.name, option.value));
        }
        input
    }
}

impl CommandRegistry {
    pub fn component_selection_prompt(
        &self,
        input: &str,
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else {
            return Ok(None);
        };
        let Some(index) = self.lookup.get(&normalize_command_name(name)) else {
            return Ok(None);
        };
        self.commands[*index].component_selection_prompt(&words.collect::<Vec<_>>())
    }
}
