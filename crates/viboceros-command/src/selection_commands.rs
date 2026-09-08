//! Transient selection commands and their command-line argument policies.

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_commands_preserve_geometry_and_both_history_stacks() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        let redo = document.redo_label().map(str::to_owned);
        assert!(undo.is_some() && redo.is_some());
        for command in [
            "SelAll",
            "SelNone",
            "Invert",
            "SelLast",
            "SelPrev",
            "SelName missing",
            "SelColor 1,2,3",
            "SelLayer Default",
            "SelGroup missing",
            "SelDup",
            "SelDupAll",
        ] {
            registry.execute(&mut document, command).unwrap();
            assert_eq!(
                document.objects().cloned().collect::<Vec<_>>(),
                original,
                "{command}"
            );
            assert_eq!(document.undo_label(), undo.as_deref(), "{command}");
            assert_eq!(document.redo_label(), redo.as_deref(), "{command}");
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 2);
        assert!(document.objects().any(
            |object| matches!(object.geometry(), Geometry::Point(p) if p.to_array() == [1., 0., 0.])
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 0);
    }
}

pub(super) struct SelAllCommand;

impl Command for SelAllCommand {
    fn name(&self) -> &'static str {
        "SelAll"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SelAll")?;
        let count = document.select_all();
        Ok(format!("Selected {count} object(s)"))
    }
}

pub(super) struct SelNoneCommand;

impl Command for SelNoneCommand {
    fn name(&self) -> &'static str {
        "SelNone"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SelNone")?;
        let count = document.clear_selection();
        Ok(format!("Deselected {count} object(s)"))
    }
}

pub(super) struct InvertCommand;

impl Command for InvertCommand {
    fn name(&self) -> &'static str {
        "Invert"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Invert")?;
        let count = document.invert_selection();
        Ok(format!("Selected {count} object(s)"))
    }
}

#[derive(Default)]
pub(super) struct SelLastCommand {
    deselect_others: remembered::Remembered<Option<bool>>,
}

impl Command for SelLastCommand {
    fn name(&self) -> &'static str {
        "SelLast"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let deselect_others = parse_action_selection_arguments(
            arguments,
            "SelLast [DeselectOthersBeforeSelect=Yes|No]",
            &self.deselect_others,
        )?;
        let count = document.select_last_changed(deselect_others);
        Ok(format!("Selection contains {count} object(s)"))
    }
}

#[derive(Default)]
pub(super) struct SelPrevCommand {
    deselect_others: remembered::Remembered<Option<bool>>,
}

impl Command for SelPrevCommand {
    fn name(&self) -> &'static str {
        "SelPrev"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let deselect_others = parse_action_selection_arguments(
            arguments,
            "SelPrev [DeselectOthersBeforeSelect=Yes|No]",
            &self.deselect_others,
        )?;
        let count = document.select_previous(deselect_others);
        Ok(format!("Selection contains {count} object(s)"))
    }
}

fn parse_action_selection_arguments(
    arguments: &[&str],
    usage: &'static str,
    remembered: &remembered::Remembered<Option<bool>>,
) -> Result<bool, CommandError> {
    if arguments.is_empty() {
        return Ok(remembered.get().unwrap_or(true));
    }
    let (name, value) = match arguments {
        [option] => option.split_once('=').ok_or(CommandError::Usage(usage))?,
        [name, value] => (*name, *value),
        _ => return Err(CommandError::Usage(usage)),
    };
    if !name
        .trim_start_matches('_')
        .eq_ignore_ascii_case("DeselectOthersBeforeSelect")
    {
        return Err(CommandError::Usage(usage));
    }
    let value = parse_yes_no(value.trim_start_matches('_')).ok_or(CommandError::Usage(usage))?;
    remembered.set(Some(value));
    Ok(value)
}

pub(super) struct SelNameCommand;

impl Command for SelNameCommand {
    fn name(&self) -> &'static str {
        "SelName"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let pattern = parse_attribute_pattern(arguments, "SelName name-pattern")?;
        let count = document.select_objects_by_name_pattern(&pattern);
        Ok(format!("Selected {count} object(s)"))
    }
}

pub(super) struct SelColorCommand;

impl Command for SelColorCommand {
    fn name(&self) -> &'static str {
        "SelColor"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [value] = arguments else {
            return Err(CommandError::Usage("SelColor r,g,b"));
        };
        let color = parse_color(value)?;
        let count = document.select_objects_by_display_color(color)?;
        Ok(format!(
            "Selected {count} object(s) with display color {},{},{}",
            color.red, color.green, color.blue
        ))
    }
}

pub(super) struct SelLayerCommand;

impl Command for SelLayerCommand {
    fn name(&self) -> &'static str {
        "SelLayer"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let pattern = parse_attribute_pattern(arguments, "SelLayer layer-pattern")?;
        let count = document.select_layer_objects_by_name_pattern(&pattern)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

pub(super) struct SelGroupCommand;

impl Command for SelGroupCommand {
    fn name(&self) -> &'static str {
        "SelGroup"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let name = parse_attribute_pattern(arguments, "SelGroup group-name")?;
        let count = document.select_group_objects_by_name(&name);
        Ok(format!("Selected {count} object(s)"))
    }
}

pub(super) struct SelectDuplicateCommand {
    pub(super) name: &'static str,
    pub(super) include_originals: bool,
}

impl Command for SelectDuplicateCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name)?;
        let count = document.select_duplicate_objects(self.include_originals)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

fn parse_attribute_pattern(
    arguments: &[&str],
    usage: &'static str,
) -> Result<String, CommandError> {
    if arguments.is_empty() {
        return Err(CommandError::Usage(usage));
    }
    let joined = arguments.join(" ");
    let pattern = joined.trim();
    let starts_quoted = pattern.starts_with('"');
    let ends_quoted = pattern.ends_with('"');
    if starts_quoted != ends_quoted {
        return Err(CommandError::Usage(usage));
    }
    Ok(if starts_quoted && pattern.len() >= 2 {
        pattern[1..pattern.len() - 1].to_owned()
    } else {
        pattern.to_owned()
    })
}
