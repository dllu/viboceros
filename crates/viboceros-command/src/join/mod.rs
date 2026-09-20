//! Object-family dispatch and preferences; geometry algorithms stay in the kernel.
use super::*;
mod curves;
mod meshes;
#[cfg(test)]
mod tests;

pub(super) struct JoinCommand {
    disjoint: remembered::Remembered<bool>,
    copy_inputs: bool,
}

impl Default for JoinCommand {
    fn default() -> Self {
        Self {
            disjoint: remembered::Remembered::new(true),
            copy_inputs: false,
        }
    }
}

impl Command for JoinCommand {
    fn name(&self) -> &'static str {
        if self.copy_inputs { "JoinCopy" } else { "Join" }
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.execute(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(document, arguments, true)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Join,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![BooleanSelectionOption {
                name: "JoinDisjointMeshes",
                value: self.parse(arguments)?,
                aliases: &[],
            }],
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.disjoint.set(self.parse(arguments)?);
        Ok(())
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(error, CommandError::NoOpenCurvesToJoin) {
            document.clear_selection();
        }
    }
}

impl JoinCommand {
    pub(super) fn copy() -> Self {
        Self {
            copy_inputs: true,
            ..Self::default()
        }
    }

    fn parse(&self, arguments: &[&str]) -> Result<bool, CommandError> {
        if arguments.is_empty() {
            return Ok(self.disjoint.get());
        }
        let usage = if self.copy_inputs {
            "JoinCopy [JoinDisjointMeshes=Yes|No]"
        } else {
            "Join [JoinDisjointMeshes=Yes|No]"
        };
        let (name, value, consumed) = orient_option(arguments, 0, usage)?;
        require_consumed(arguments, consumed, usage)?;
        if !option_name_eq(name, "JoinDisjointMeshes") {
            return Err(CommandError::Usage(usage));
        }
        parse_yes_no(value).ok_or(CommandError::Usage(usage))
    }

    fn execute(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let disjoint = self.parse(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let sources = if postselected {
            document.selected_objects().collect::<Vec<_>>()
        } else {
            // Rhino's preselection scan uses document table order; command-
            // first selection retains the individual pick order.
            document
                .objects()
                .filter(|o| document.is_selected(o.id()))
                .collect::<Vec<_>>()
        };
        let plan = if sources
            .iter()
            .all(|o| matches!(o.geometry(), Geometry::Mesh(_)))
        {
            meshes::stage(&sources, document.tolerance(), disjoint)?
        } else {
            curves::stage(&sources, document.tolerance(), postselected)?
        };
        let outputs = document.copy_object_geometries_into_source_groups_in_order(plan.copies)?;
        if !self.copy_inputs {
            document.delete_objects(plan.consumed)?;
        }
        if postselected {
            document.clear_selection();
        } else {
            // Keep selected originals/singletons without expanding their groups.
            document.select_objects_direct(outputs, SelectionMode::Add)?;
        }
        self.disjoint.set(disjoint);
        Ok(plan.description)
    }
}

/// All geometry is validated before the shared, transactional document edit.
struct JoinPlan {
    copies: Vec<(ObjectId, Geometry)>,
    consumed: Vec<ObjectId>,
    description: String,
}
