//! Rebind selected roots of one block definition to a new shared duplicate.
use super::*;

pub(super) struct CreateUniqueBlockCommand;
impl Command for CreateUniqueBlockCommand {
    fn name(&self) -> &'static str {
        "CreateUniqueBlock"
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        blocks::tokenize(input)
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if arguments.len() > 1 {
            return Err(CommandError::Usage("CreateUniqueBlock [new-name]"));
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "CreateUniqueBlock",
            filter: ObjectSelectionFilter::Blocks,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: if arguments.is_empty() {
                ObjectSelectionWorkflow::PointInputAfterSelection
            } else {
                ObjectSelectionWorkflow::OptionsDuringSelection
            },
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [name] = arguments else {
            return Err(CommandError::Usage("CreateUniqueBlock new-name"));
        };
        let ids = document
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = ids.len();
        let id = document.make_block_instances_unique(*name, ids)?;
        Ok(format!(
            "Made {count} instance(s) unique as '{}'",
            document.block_definition(id).unwrap().name()
        ))
    }
}

#[cfg(test)]
mod tests;
