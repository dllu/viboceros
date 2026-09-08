//! Command-owned object filters and typed options for selection prompts.
use super::*;

mod choice;
pub use choice::{ChoiceSelectionOption, SelectionToggle};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectSelectionFilter {
    #[default]
    Any,
    Mesh,
    ToNurbs,
    Beziers,
    Surfaces,
}

impl ObjectSelectionFilter {
    pub fn accepts(self, geometry: &Geometry) -> bool {
        match self {
            Self::Any => true,
            Self::Mesh => matches!(geometry, Geometry::Mesh(_)),
            Self::ToNurbs => !matches!(geometry, Geometry::Point(_) | Geometry::PointCloud(_)),
            Self::Beziers => geometry.curve_ref().is_some() || Self::Surfaces.accepts(geometry),
            Self::Surfaces => {
                matches!(geometry, Geometry::NurbsSurface(_))
                    || matches!(geometry, Geometry::Brep(brep) if brep.faces().len() == 1)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooleanSelectionOption {
    pub name: &'static str,
    pub value: bool,
    pub aliases: &'static [&'static str],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooleanSelectionMenu {
    pub name: &'static str,
    pub options: Vec<BooleanSelectionOption>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectSelectionWorkflow {
    OptionsDuringSelection,
    ConfirmAfterSelection,
    /// A single Yes/No answer executes immediately; Enter uses its current value.
    ChooseBooleanAfterSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectSelectionPrompt {
    pub command: &'static str,
    pub filter: ObjectSelectionFilter,
    pub options: Vec<BooleanSelectionOption>,
    pub menus: Vec<BooleanSelectionMenu>,
    pub choices: Vec<ChoiceSelectionOption>,
    pub workflow: ObjectSelectionWorkflow,
}

impl ObjectSelectionPrompt {
    /// Stage an entire input before accepting any option, including duplicates.
    pub fn update_options(&mut self, input: &str) -> Result<(), CommandError> {
        if self.workflow == ObjectSelectionWorkflow::ChooseBooleanAfterSelection
            && let [option] = self.options.as_mut_slice()
            && let Some(value) = parse_yes_no(input.trim())
        {
            option.value = value;
            return Ok(());
        }
        const USAGE: &str = "known-option=value [known-option=value ...] or an available action";
        let arguments = input.split_whitespace().collect::<Vec<_>>();
        if arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut staged = self.clone();
        let mut seen = BTreeSet::new();
        let mut menus_seen = BTreeSet::new();
        let mut changed = false;
        let mut i = 0;
        while i < arguments.len() {
            if let Some(choice) = staged.choices.iter_mut().find(|c| {
                c.toggle
                    .as_ref()
                    .is_some_and(|t| option_name_eq(arguments[i], t.name))
            }) {
                choice.toggle()?;
                changed = true;
                i += 1;
                continue;
            }
            if let Some(menu) = staged
                .menus
                .iter()
                .find(|m| option_name_eq(arguments[i], m.name))
            {
                if !menus_seen.insert(menu.name) {
                    return Err(CommandError::Usage(USAGE));
                }
                i += 1;
                continue;
            }
            let (name, value, consumed) = orient_option(&arguments, i, USAGE)?;
            if let Some(choice) = staged
                .choices
                .iter_mut()
                .find(|c| option_name_eq(name, c.name))
            {
                if !seen.insert(choice.name) {
                    return Err(CommandError::Usage(USAGE));
                }
                choice.set(value)?;
                changed = true;
                i += consumed;
                continue;
            }
            let option = staged
                .options
                .iter_mut()
                .chain(staged.menus.iter_mut().flat_map(|m| &mut m.options))
                .find(|o| {
                    option_name_eq(name, o.name)
                        || o.aliases.iter().any(|alias| option_name_eq(name, alias))
                })
                .ok_or(CommandError::Usage(USAGE))?;
            if !seen.insert(option.name) {
                return Err(CommandError::Usage(USAGE));
            }
            option.value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            changed = true;
            i += consumed;
        }
        if !changed {
            return Err(CommandError::Usage(USAGE));
        }
        *self = staged;
        Ok(())
    }

    pub fn update_menu_options(&mut self, menu: usize, input: &str) -> Result<(), CommandError> {
        let options = self
            .menus
            .get(menu)
            .ok_or(CommandError::Usage("known option submenu"))?
            .options
            .clone();
        let mut scoped = Self {
            options,
            menus: vec![],
            choices: vec![],
            ..self.clone()
        };
        scoped.update_options(input)?;
        self.menus[menu].options = scoped.options;
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
        for choice in &self.choices {
            input.push_str(&format!(" {}={}", choice.name, choice.value));
        }
        for menu in &self.menus {
            input.push_str(&format!(" {}", menu.name));
            for option in &menu.options {
                input.push_str(&format!(
                    " {}={}",
                    option.name,
                    if option.value { "Yes" } else { "No" }
                ));
            }
        }
        input
    }
}

impl CommandRegistry {
    pub fn object_selection_confirmation(
        &self,
        document: &Document,
        prompt: &ObjectSelectionPrompt,
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = prompt.command_line();
        let mut tokens = input.split_whitespace();
        let name = normalize_command_name(tokens.next().ok_or(CommandError::EmptyInput)?);
        let index = self
            .lookup
            .get(&name)
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        self.commands[*index].object_selection_confirmation(document, &tokens.collect::<Vec<_>>())
    }
    pub fn accept_object_selection_options(
        &self,
        prompt: &ObjectSelectionPrompt,
    ) -> Result<(), CommandError> {
        let input = prompt.command_line();
        self.accept_object_selection_input(&input)
    }

    /// Accepts a prompt's explicit choices without requiring a UI descriptor.
    /// Commands with confirmation-time memory may intentionally do nothing.
    pub fn accept_object_selection_input(&self, input: &str) -> Result<(), CommandError> {
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
