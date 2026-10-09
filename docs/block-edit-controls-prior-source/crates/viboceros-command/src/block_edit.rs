//! Scriptable in-place block-edit lifecycle.
use super::*;
pub(super) struct BlockEditCommand;
impl Command for BlockEditCommand {
    fn name(&self) -> &'static str {
        "BlockEdit"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok((args.is_empty()
            || args.len() == 1 && args[0].trim_start_matches('_').eq_ignore_ascii_case("Open"))
        .then(|| ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Blocks,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        if args.len() == 1
            && args[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("DiscardAndCancel")
        {
            doc.discard_block_edit()?;
            return Ok("Discarded block edits".into());
        }
        if args.len() == 1
            && args[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("SaveAndClose")
        {
            let count = doc.save_block_edit()?;
            return Ok(format!("Saved block definition with {count} member(s)"));
        }
        let args = if args
            .first()
            .is_some_and(|s| s.trim_start_matches('_').eq_ignore_ascii_case("Open"))
        {
            &args[1..]
        } else {
            args
        };
        let target = match args {
            [] => {
                let ids = doc
                    .selected_objects()
                    .filter(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                if ids.len() != 1 {
                    return Err(CommandError::Usage("select one block instance to edit"));
                }
                ids[0]
            }
            [id] => id.parse::<ObjectId>().map_err(|_| {
                CommandError::Usage(
                    "BlockEdit [Open] [object-id] | SaveAndClose | DiscardAndCancel",
                )
            })?,
            _ => {
                return Err(CommandError::Usage(
                    "BlockEdit [Open] [object-id] | SaveAndClose | DiscardAndCancel",
                ));
            }
        };
        let ids = doc.open_block_edit(target)?;
        Ok(format!(
            "Editing block in place: {} member(s); BlockEdit SaveAndClose accepts, DiscardAndCancel discards",
            ids.len()
        ))
    }
}

#[cfg(test)]
mod tests;
