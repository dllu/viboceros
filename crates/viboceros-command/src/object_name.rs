//! Object naming and quote-aware command arguments.

use super::*;

pub(super) const SET_OBJECT_NAME_USAGE: &str = "SetObjectName name [AppendCounter=Yes|No]";

pub(super) struct SetObjectNameCommand;

impl Command for SetObjectNameCommand {
    fn name(&self) -> &'static str {
        "SetObjectName"
    }

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        tokenize_name_arguments(input)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (name, append_counter) = parse_set_object_name_arguments(arguments)?;
        let selected = document
            .objects()
            .filter(|object| document.is_selected(object.id()))
            .map(|object| object.id())
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let has_name = name.is_some();
        let assignments = match name.as_ref() {
            Some(name) if append_counter => selected
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, Some(format!("{name} {index}"))))
                .collect::<Vec<_>>(),
            _ => selected
                .iter()
                .map(|id| (*id, name.clone()))
                .collect::<Vec<_>>(),
        };
        document.set_object_names(assignments)?;
        Ok(if has_name {
            format!("Named {} object(s)", selected.len())
        } else {
            format!("Cleared names on {} object(s)", selected.len())
        })
    }
}

// Retain quotes so the semantic parser can distinguish literal option-like
// names from options. All slicing offsets come from UTF-8 character boundaries.
fn tokenize_name_arguments(mut input: &str) -> Result<Vec<&str>, CommandError> {
    let mut arguments = Vec::new();
    loop {
        input = input.trim_start();
        if input.is_empty() {
            return Ok(arguments);
        }
        let end = if let Some(tail) = input.strip_prefix('"') {
            let end = tail
                .find('"')
                .ok_or(CommandError::Usage(SET_OBJECT_NAME_USAGE))?
                + 2;
            if input[end..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace())
            {
                return Err(CommandError::Usage(SET_OBJECT_NAME_USAGE));
            }
            end
        } else {
            let end = input.find(char::is_whitespace).unwrap_or(input.len());
            if input[..end].contains('"') {
                return Err(CommandError::Usage(SET_OBJECT_NAME_USAGE));
            }
            end
        };
        arguments.push(&input[..end]);
        input = &input[end..];
    }
}

fn parse_set_object_name_arguments(
    arguments: &[&str],
) -> Result<(Option<String>, bool), CommandError> {
    let mut append_counter = false;
    let mut name_parts = Vec::new();
    for argument in arguments {
        if let Some(quoted) = argument.strip_prefix('"') {
            let literal = quoted
                .strip_suffix('"')
                .ok_or(CommandError::Usage(SET_OBJECT_NAME_USAGE))?;
            name_parts.push(literal);
            continue;
        }
        let normalized = argument.trim_start_matches('_');
        if let Some((option, value)) = normalized.split_once('=')
            && option.eq_ignore_ascii_case("AppendCounter")
        {
            append_counter = parse_yes_no(value.trim_start_matches('_'))
                .ok_or(CommandError::Usage(SET_OBJECT_NAME_USAGE))?;
        } else {
            name_parts.push(*argument);
        }
    }
    if name_parts.is_empty() {
        return Err(CommandError::Usage(SET_OBJECT_NAME_USAGE));
    }
    let joined = name_parts.join(" ");
    let trimmed = joined.trim();
    let name = (!trimmed.is_empty()).then(|| trimmed.to_owned());
    Ok((name, append_counter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_options_are_recognized_only_outside_quotes() {
        for (input, expected, counter) in [
            ("_AppendCounter=_Yes \"Part  A\"", Some("Part  A"), true),
            ("\"Part  A\" AppendCounter=Yes", Some("Part  A"), true),
            (
                "\"AppendCounter=Maybe\"",
                Some("AppendCounter=Maybe"),
                false,
            ),
            (
                "\"Part AppendCounter=Yes A\"",
                Some("Part AppendCounter=Yes A"),
                false,
            ),
            ("Shared Name", Some("Shared Name"), false),
            ("\"\" AppendCounter=Yes", None, true),
            ("\" \t \"", None, false),
        ] {
            let arguments = tokenize_name_arguments(input).unwrap();
            let (name, append_counter) = parse_set_object_name_arguments(&arguments).unwrap();
            assert_eq!(name.as_deref(), expected, "{input:?}");
            assert_eq!(append_counter, counter, "{input:?}");
        }
    }

    #[test]
    fn malformed_names_leave_objects_selection_and_history_unchanged() {
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
        for input in [
            "",
            "\"",
            "\"Part",
            "Part\"",
            "\"Part\"tail",
            "\"Part\"\"A\"",
            "AppendCounter=Yes",
            "\"Part\" AppendCounter=Maybe",
        ] {
            assert!(
                matches!(
                    registry.execute(&mut document, &format!("SetObjectName {input}")),
                    Err(CommandError::Usage(_))
                ),
                "{input:?}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), selected);
            assert_eq!(document.undo_label(), undo.as_deref());
            assert_eq!(document.redo_label(), redo.as_deref());
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn quoted_names_preserve_whitespace_and_literal_options_through_history() {
        let registry = CommandRegistry::with_builtins();
        for name in [
            "Part  A",
            "Part\tA",
            "部品\u{2003}A",
            "Part AppendCounter=Yes A",
            "AppendCounter=Maybe",
        ] {
            let mut document = Document::default();
            registry.execute(&mut document, "Point 0,0,0").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            let id = document.selected_object_ids().next().unwrap();
            registry
                .execute(&mut document, &format!("SetObjectName \"{name}\""))
                .unwrap();
            assert_eq!(document.object(id).unwrap().attributes().name(), Some(name));
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.object(id).unwrap().attributes().name(), None);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.object(id).unwrap().attributes().name(), Some(name));
            registry.execute(&mut document, "SelNone").unwrap();
            registry
                .execute(&mut document, &format!("SelName \"{name}\""))
                .unwrap();
            assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
        }
    }
}
