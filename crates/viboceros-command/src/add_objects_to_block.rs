//! Append selected or explicitly identified model objects to a block instance.
use super::*;

pub(super) struct AddObjectsToBlockCommand;
impl Command for AddObjectsToBlockCommand {
    fn name(&self) -> &'static str {
        "AddObjectsToBlock"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (target, sources) = arguments.split_first().ok_or(CommandError::Usage(
            "AddObjectsToBlock target-object-id [source-object-id ...]",
        ))?;
        let target = target
            .parse::<ObjectId>()
            .map_err(|_| CommandError::Usage("expected a target block instance ID"))?;
        let sources = if sources.is_empty() {
            document
                .selected_object_ids()
                .filter(|id| *id != target)
                .collect::<Vec<_>>()
        } else {
            sources
                .iter()
                .map(|s| {
                    s.parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage("expected source object IDs"))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.add_objects_to_block(target, sources)?;
        Ok(format!("Added {count} object(s) to the block definition"))
    }
}

#[cfg(test)]
mod tests;
