//! Definition replacement with explicit selected-only or all-root scope.
use super::*;

pub(super) struct ReplaceBlockCommand;
pub const USAGE: &str = "ReplaceBlock [All|None] [BlockDefinitionName=]target-name";
pub fn options<'a>(arguments: &[&'a str]) -> Result<(bool, Option<&'a str>), CommandError> {
    let mut all = false;
    let mut seen_scope = false;
    let mut target = None;
    for argument in arguments {
        let keyword = argument.trim_start_matches('_');
        if keyword.eq_ignore_ascii_case("All") || keyword.eq_ignore_ascii_case("None") {
            if seen_scope {
                return Err(CommandError::Usage(USAGE));
            }
            seen_scope = true;
            all = keyword.eq_ignore_ascii_case("All");
        } else {
            let name = if let Some((key, name)) = argument.split_once('=') {
                if !key
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("BlockDefinitionName")
                {
                    return Err(CommandError::Usage(USAGE));
                }
                name
            } else {
                argument
            };
            if name.is_empty() || target.replace(name).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    Ok((all, target))
}
impl Command for ReplaceBlockCommand {
    fn name(&self) -> &'static str {
        "ReplaceBlock"
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        blocks::tokenize(input)
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (_, name) = options(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "ReplaceBlock",
            filter: ObjectSelectionFilter::Blocks,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: if name.is_some() {
                ObjectSelectionWorkflow::OptionsDuringSelection
            } else {
                ObjectSelectionWorkflow::PointInputAfterSelection
            },
        }))
    }
    fn run(&self, doc: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (all, name) = options(arguments)?;
        let name = name.ok_or(CommandError::Usage(USAGE))?;
        let target = doc
            .block_definition_by_name(name)
            .ok_or(CommandError::Usage(
                "replacement block definition was not found",
            ))?
            .id();
        let ids = doc
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = doc.replace_block_instances(target, ids, all)?;
        Ok(format!("Replaced {count} instance(s) with '{name}'"))
    }
}

#[cfg(test)]
mod tests;
