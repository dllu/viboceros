//! Whole-SubD face packing, staged before a single document history edit.
use crate::*;

pub(super) struct PackSubDFaces;

impl Command for PackSubDFaces {
    fn name(&self) -> &'static str {
        "PackSubDFaces"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, self.name())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::SubD,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.apply(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.apply(doc, args, true)
    }
}

impl PackSubDFaces {
    fn apply(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(args, 0, self.name())?;
        if doc.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut staged = Vec::new();
        let mut bytes = 0usize;
        for object in doc.selected_objects() {
            let Geometry::SubD(source) = object.geometry() else {
                continue;
            };
            if !doc.is_object_selectable(object.id()) {
                return Err(CommandError::FacePackingUnavailable);
            }
            let output = source.try_repacked_faces()?;
            bytes = bytes.saturating_add(output.archive().len());
            if bytes > 64 * 1024 * 1024 {
                return Err(CommandError::FacePackingResourceLimit);
            }
            staged.push((object.id(), Geometry::SubD(output)));
        }
        if staged.is_empty() {
            return Err(CommandError::FacePackingUnavailable);
        }
        if postselected {
            let ids = doc.selected_object_ids().collect::<Vec<_>>();
            doc.release_command_selection_on_history_replay(ids)?;
            doc.clear_selection();
        }
        let ids = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        doc.replace_object_geometries_with_history(
            staged,
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        doc.move_objects_to_end_in_order(ids.iter().copied())?;
        Ok(format!("Packed faces in {} SubD object(s)", ids.len()))
    }
}
