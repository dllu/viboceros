//! Recursive block expansion, separate from ordinary one-level Explode.
use super::*;

pub(super) struct ExplodeBlockCommand;

fn options(arguments: &[&str]) -> Result<(bool, bool), CommandError> {
    let mut all = false;
    let mut group = false;
    let mut seen = BTreeSet::new();
    for argument in arguments {
        let (name, value) = argument
            .split_once('=')
            .map_or((*argument, "Yes"), |(a, b)| (a, b));
        let name = name.trim_start_matches('_').to_ascii_lowercase();
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(
                "ExplodeBlock [AllBlocks] [GroupOutput=Yes|No]",
            ));
        }
        let value = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "yes" => true,
            "no" => false,
            _ => {
                return Err(CommandError::Usage(
                    "ExplodeBlock [AllBlocks] [GroupOutput=Yes|No]",
                ));
            }
        };
        match name.as_str() {
            "allblocks" => all = value,
            "groupoutput" => group = value,
            _ => {
                return Err(CommandError::Usage(
                    "ExplodeBlock [AllBlocks] [GroupOutput=Yes|No]",
                ));
            }
        }
    }
    Ok((all, group))
}

impl Command for ExplodeBlockCommand {
    fn name(&self) -> &'static str {
        "ExplodeBlock"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (all, group) = options(arguments)?;
        Ok((!all).then(|| ObjectSelectionPrompt {
            command: "ExplodeBlock",
            filter: ObjectSelectionFilter::Blocks,
            options: vec![BooleanSelectionOption {
                name: "GroupOutput",
                value: group,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (all, group) = options(arguments)?;
        let ids = if all {
            document
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::BlockInstance(_)))
                .map(|object| object.id())
                .collect::<Vec<_>>()
        } else {
            document
                .selected_objects()
                .filter(|object| matches!(object.geometry(), Geometry::BlockInstance(_)))
                .map(|object| object.id())
                .collect()
        };
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        run_with_budget(document, ids, group, MAX_SPAN_OUTPUT_OBJECTS)
    }
}

fn run_with_budget(
    document: &mut Document,
    ids: Vec<ObjectId>,
    group: bool,
    maximum: usize,
) -> Result<String, CommandError> {
    let mut remaining = maximum;
    let mut prepared = Vec::new();
    for id in ids {
        let plan = document.prepare_block_explosion(id, true, remaining)?;
        remaining -= plan.output_count();
        prepared.push(plan);
    }
    let sources = prepared.len();
    let outputs = document.commit_block_explosions(prepared, group)?;
    Ok(format!(
        "Exploded {sources} block instance(s) into {} object(s)",
        outputs.len()
    ))
}

#[cfg(test)]
mod tests;
