//! Attribute user text commands.

use super::*;

const SET_USAGE: &str = "SetUserText key value [AttachTo=Attributes]";
const KEY_VALUE_USAGE: &str = "SelKeyValue key-pattern value-pattern";

fn arguments<'a>(input: &'a str, usage: &'static str) -> Result<Vec<&'a str>, CommandError> {
    object_name::tokenize_name_arguments(input, usage)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(value)
}

pub(super) struct SetUserTextCommand;

pub(super) struct GetUserTextCommand;

impl Command for GetUserTextCommand {
    fn name(&self) -> &'static str {
        "GetUserText"
    }

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        arguments(input, "GetUserText [key]")
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.len() > 1 {
            return Err(CommandError::Usage("GetUserText [key]"));
        }
        let id = document
            .selected_object_ids()
            .next()
            .ok_or(CommandError::NoObjectsSelected)?;
        let text = document
            .object(id)
            .expect("selected object exists")
            .attributes()
            .user_text();
        if let Some(key) = arguments.first() {
            let key = unquote(key);
            return Ok(text
                .iter()
                .find(|(candidate, _)| candidate.to_lowercase() == key.to_lowercase())
                .map_or_else(String::new, |(_, value)| value.clone()));
        }
        Ok(text
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

impl Command for SetUserTextCommand {
    fn name(&self) -> &'static str {
        "SetUserText"
    }

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        arguments(input, SET_USAGE)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let arguments = arguments
            .iter()
            .copied()
            .filter(|arg| !arg.eq_ignore_ascii_case("AttachTo=Attributes"))
            .collect::<Vec<_>>();
        let [key, value] = arguments.as_slice() else {
            return Err(CommandError::Usage(SET_USAGE));
        };
        let key = unquote(key);
        let value = unquote(value);
        if key.is_empty() {
            return Err(CommandError::Usage(SET_USAGE));
        }
        let ids = document.selected_object_ids().collect::<Vec<_>>();
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count =
            document.set_object_user_text(ids, key, (!value.is_empty()).then_some(value))?;
        Ok(format!("Updated user text on {count} object(s)"))
    }
}

pub(super) struct SelUserTextCommand {
    pub name: &'static str,
}

impl Command for SelUserTextCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn records_history(&self) -> bool {
        false
    }

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        arguments(
            input,
            if self.name == "SelKeyValue" {
                KEY_VALUE_USAGE
            } else {
                self.name
            },
        )
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (key, value) = match (self.name, arguments) {
            ("SelKey", [key]) => (Some(unquote(key)), None),
            ("SelValue", [value]) => (None, Some(unquote(value))),
            ("SelKeyValue", [key, value]) => (Some(unquote(key)), Some(unquote(value))),
            _ => {
                return Err(CommandError::Usage(if self.name == "SelKeyValue" {
                    KEY_VALUE_USAGE
                } else {
                    self.name
                }));
            }
        };
        let count = document.select_objects_by_user_text(key, value);
        Ok(format!("Selected {count} object(s)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_commands_select_by_matching_pair_and_support_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "SetUserText \"Part Number\" \"A 12\"")
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "GetUserText \"part number\"")
                .unwrap(),
            "A 12"
        );
        document
            .select_object(second, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "SetUserText \"Part Number\" B12")
            .unwrap();
        registry
            .execute(&mut document, "SetUserText \"part number\" B13")
            .unwrap();
        assert_eq!(
            document
                .object(second)
                .unwrap()
                .attributes()
                .user_text()
                .len(),
            1
        );
        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "SelKeyValue \"part number\" \"a ?2\"")
                .unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![first]
        );
        registry
            .execute(&mut document, "SetUserText \"Part Number\" \"\"")
            .unwrap();
        assert!(
            document
                .object(first)
                .unwrap()
                .attributes()
                .user_text()
                .is_empty()
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(first).unwrap().attributes().user_text()["Part Number"],
            "A 12"
        );
    }
}
