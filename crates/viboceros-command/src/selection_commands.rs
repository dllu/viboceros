//! Transient selection commands and their command-line argument policies.

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_selection_preserves_internal_name_whitespace() {
        let registry = CommandRegistry::with_builtins();
        for name in ["Part  A", "Part\tA", "部品\u{2003}A"] {
            let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut document = Document::default();
            let mut ids = Vec::new();
            for (index, name) in [name, collapsed.as_str()].into_iter().enumerate() {
                let layer = document.add_layer(name, ColorRgb::new(1, 2, 3)).unwrap();
                let id = document
                    .add_geometry_with_attributes(
                        Geometry::Point(Point3::try_new(index as f64, 0.0, 0.0).unwrap()),
                        ObjectAttributes::on_layer(layer).with_name(name),
                    )
                    .unwrap();
                document.add_group(Some(name.to_owned()), [id]).unwrap();
                ids.push(id);
            }
            let original = document.objects().cloned().collect::<Vec<_>>();
            let undo = document.undo_label().map(str::to_owned);
            for command in ["SelName", "SelLayer", "SelGroup"] {
                for argument in [name.to_owned(), format!("\"{name}\"")] {
                    document.clear_selection();
                    let input = format!("  _{command}\t {argument}  ");
                    registry.execute(&mut document, &input).unwrap();
                    assert_eq!(
                        document.selected_object_ids().collect::<Vec<_>>(),
                        [ids[0]],
                        "{input:?}"
                    );
                    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
                    assert_eq!(document.undo_label(), undo.as_deref());
                }
            }
        }
    }

    #[test]
    fn attribute_patterns_require_distinct_opening_and_closing_quotes() {
        for (arguments, expected) in [
            (vec!["*part?"], "*part?"),
            (vec!["two", "words"], "two words"),
            (vec!["\"two", "words\""], "two words"),
            (vec!["\"\""], ""),
            (vec!["\"部品\""], "部品"),
        ] {
            assert_eq!(
                parse_attribute_pattern(&arguments, "usage").unwrap(),
                expected
            );
        }
        for arguments in [vec![], vec!["\""], vec!["\"part"], vec!["part\""]] {
            assert!(matches!(
                parse_attribute_pattern(&arguments, "usage"),
                Err(CommandError::Usage("usage"))
            ));
        }
    }

    #[test]
    fn malformed_attribute_patterns_preserve_selection_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        let redo = document.redo_label().map(str::to_owned);
        assert!(!selected.is_empty() && undo.is_some() && redo.is_some());
        for command in ["SelName", "SelLayer", "SelGroup"] {
            for pattern in ["", "\"", "\"part", "part\""] {
                let input = format!("{command} {pattern}");
                assert!(
                    matches!(
                        registry.execute(&mut document, &input),
                        Err(CommandError::Usage(_))
                    ),
                    "{input}"
                );
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
                assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), selected);
                assert_eq!(document.undo_label(), undo.as_deref());
                assert_eq!(document.redo_label(), redo.as_deref());
            }
        }
    }

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

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        Ok(attribute_pattern_arguments(input))
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

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        Ok(attribute_pattern_arguments(input))
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

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        Ok(attribute_pattern_arguments(input))
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

// These commands take one name/pattern, not a sequence of whitespace-delimited
// options. Keep its internal whitespace so imported names remain addressable.
fn attribute_pattern_arguments(input: &str) -> Vec<&str> {
    let input = input.trim();
    if input.is_empty() {
        Vec::new()
    } else {
        vec![input]
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
    if starts_quoted != ends_quoted || (starts_quoted && pattern.len() < 2) {
        return Err(CommandError::Usage(usage));
    }
    Ok(if starts_quoted && pattern.len() >= 2 {
        pattern[1..pattern.len() - 1].to_owned()
    } else {
        pattern.to_owned()
    })
}
