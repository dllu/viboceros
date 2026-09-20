//! Object-family dispatch and preferences; geometry algorithms stay in the kernel.
use super::*;
mod curves;
#[cfg(test)]
mod tests;

const USAGE: &str = "Join [JoinDisjointMeshes=Yes|No]";

pub(super) struct JoinCommand {
    disjoint: remembered::Remembered<bool>,
}

impl Default for JoinCommand {
    fn default() -> Self {
        Self {
            disjoint: remembered::Remembered::new(true),
        }
    }
}

impl Command for JoinCommand {
    fn name(&self) -> &'static str {
        "Join"
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
}

impl JoinCommand {
    fn parse(&self, arguments: &[&str]) -> Result<bool, CommandError> {
        if arguments.is_empty() {
            return Ok(self.disjoint.get());
        }
        let (name, value, consumed) = orient_option(arguments, 0, USAGE)?;
        require_consumed(arguments, consumed, USAGE)?;
        if !option_name_eq(name, "JoinDisjointMeshes") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))
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
        let mesh_only = document
            .selected_objects()
            .all(|o| matches!(o.geometry(), Geometry::Mesh(_)));
        if !mesh_only {
            // The existing curve path preflights all inputs, rejecting mixed
            // object families before any document edits.
            let result = curves::run(document, &[])?;
            self.disjoint.set(disjoint);
            return Ok(result);
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
        if sources.len() == 1 {
            if postselected {
                document.clear_selection();
            }
            self.disjoint.set(disjoint);
            return Ok("One mesh unchanged".into());
        }
        let ids = sources.iter().map(|o| o.id()).collect::<Vec<_>>();
        let input_count = ids.len();
        let meshes = sources
            .iter()
            .map(|o| match o.geometry() {
                Geometry::Mesh(m) => m,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        let components = viboceros_geometry::join_meshes(
            &meshes,
            viboceros_geometry::MeshJoinOptions {
                join_disjoint: disjoint,
                alignment_tolerance: document.tolerance().absolute() * 1e-4,
                single_precision_matching: true,
            },
        )?;
        let count = components.len();
        let copies = components
            .into_iter()
            .map(|part| (ids[part.source_indices[0]], Geometry::Mesh(part.mesh)))
            .collect::<Vec<_>>();
        let outputs = document.copy_object_geometries_into_source_groups_in_order(copies)?;
        document.delete_objects(ids)?;
        document.clear_selection();
        if !postselected {
            document.select_objects_direct(outputs, SelectionMode::Replace)?;
        }
        self.disjoint.set(disjoint);
        Ok(format!(
            "Joined {input_count} mesh(es) into {count} mesh(es)"
        ))
    }
}
