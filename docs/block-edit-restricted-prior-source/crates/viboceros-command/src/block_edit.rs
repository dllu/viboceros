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
        if args.len() == 1
            && args[0]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("RemoveObject")
        {
            return Ok(Some(ObjectSelectionPrompt {
                command: self.name(),
                filter: ObjectSelectionFilter::Any,
                options: vec![],
                menus: vec![],
                choices: vec![],
                workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            }));
        }
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
        if let Some(operation) = args.first().map(|s| s.trim_start_matches('_')) {
            if operation.eq_ignore_ascii_case("EditPath") {
                if args.len() != 2 {
                    return Err(CommandError::Usage(
                        "BlockEdit EditPath Root|member/member/...",
                    ));
                }
                let path = if args[1].eq_ignore_ascii_case("Root") {
                    Vec::new()
                } else {
                    args[1]
                        .split('/')
                        .map(|part| {
                            part.parse::<usize>().map_err(|_| {
                                CommandError::Usage("BlockEdit EditPath Root|member/member/...")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                let ids = doc.switch_block_edit_context(&path)?;
                return Ok(format!(
                    "Editing nested definition: {} member(s)",
                    ids.len()
                ));
            }
            if operation.eq_ignore_ascii_case("AddObject")
                || operation.eq_ignore_ascii_case("RemoveObject")
            {
                let ids = if args.len() == 1 && operation.eq_ignore_ascii_case("RemoveObject") {
                    doc.selected_object_ids().collect::<Vec<_>>()
                } else {
                    args[1..]
                        .iter()
                        .map(|s| {
                            s.parse::<ObjectId>().map_err(|_| {
                                CommandError::Usage(
                                    "BlockEdit AddObject|RemoveObject object-id ...",
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                if ids.is_empty() {
                    return Err(CommandError::Usage(
                        "choose objects for BlockEdit AddObject or RemoveObject",
                    ));
                }
                let count = if operation.eq_ignore_ascii_case("AddObject") {
                    doc.add_objects_to_block_edit(ids)?.len()
                } else {
                    doc.remove_objects_from_block_edit(ids)?
                };
                return Ok(format!("BlockEdit {operation}: {count} object(s)"));
            }
            if operation.eq_ignore_ascii_case("SetBasePoint") {
                let (point, consumed) = parse_point(&args[1..])?;
                require_consumed(&args[1..], consumed, "BlockEdit SetBasePoint point")?;
                doc.set_block_edit_base_point(point)?;
                return Ok("Updated block-edit base point".into());
            }
        }
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
