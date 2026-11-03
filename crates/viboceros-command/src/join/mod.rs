//! Object-family dispatch and preferences; geometry algorithms stay in the kernel.
use super::*;
mod breps;
mod curves;
mod meshes;
mod subds;
#[cfg(test)]
mod tests;

pub(super) struct JoinCommand {
    disjoint: remembered::Remembered<bool>,
    crease: remembered::Remembered<bool>,
    copy_inputs: bool,
}

impl Default for JoinCommand {
    fn default() -> Self {
        Self {
            disjoint: remembered::Remembered::new(true),
            crease: remembered::Remembered::new(false),
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
        let options = self.parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Join,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![ChoiceSelectionOption {
                name: "SubDJoinedEdges",
                value: if options.crease { "Crease" } else { "Smooth" },
                choices: &["Smooth", "Crease"],
                toggle: None,
            }],
            options: vec![BooleanSelectionOption {
                name: "JoinDisjointMeshes",
                value: options.disjoint,
                aliases: &[],
            }],
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        let options = self.parse(arguments)?;
        self.disjoint.set(options.disjoint);
        self.crease.set(options.crease);
        Ok(())
    }

    fn object_selection_complete(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<bool, CommandError> {
        self.parse(arguments)?;
        let sources = document.selected_objects().collect::<Vec<_>>();
        if sources.len() < 2 || sources.iter().any(|o| o.geometry().curve_ref().is_none()) {
            return Ok(false);
        }
        match curves::stage(&sources, document.tolerance(), true, self.copy_inputs) {
            Ok(plan) => Ok(plan.closed_on_pick),
            Err(CommandError::NoOpenCurvesToJoin) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        postselected: bool,
    ) {
        if matches!(
            error,
            CommandError::NoOpenCurvesToJoin | CommandError::NoOpenSurfacesToJoin
        ) {
            document.clear_selection();
        } else if matches!(error, CommandError::NothingJoined)
            && document
                .selected_objects()
                .all(|o| breps::accepts(o.geometry()))
        {
            let open = document
                .selected_objects()
                .filter(|o| !breps::closed(o.geometry(), document.tolerance()).unwrap_or(false))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            let keep = if postselected {
                &open[..open.len().min(1)]
            } else {
                &open[..]
            };
            let _ = document.select_objects_direct(keep.iter().copied(), SelectionMode::Replace);
        } else if postselected && matches!(error, CommandError::NothingJoined) {
            let seed = document
                .selected_objects()
                .find(|object| {
                    object
                        .geometry()
                        .curve_ref()
                        .is_none_or(|c| c.is_closed() == Ok(false))
                })
                .map(|object| object.id());
            if let Some(seed) = seed {
                let _ = document.select_objects_direct([seed], SelectionMode::Replace);
            }
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

    fn parse(&self, arguments: &[&str]) -> Result<JoinOptions, CommandError> {
        let usage = if self.copy_inputs {
            "JoinCopy [JoinDisjointMeshes=Yes|No] [SubDJoinedEdges=Smooth|Crease]"
        } else {
            "Join [JoinDisjointMeshes=Yes|No] [SubDJoinedEdges=Smooth|Crease]"
        };
        let mut result = JoinOptions {
            disjoint: self.disjoint.get(),
            crease: self.crease.get(),
        };
        let mut seen = [false; 2];
        let mut index = 0;
        while index < arguments.len() {
            let (name, value, count) = orient_option(arguments, index, usage)?;
            let field = if option_name_eq(name, "JoinDisjointMeshes") {
                result.disjoint = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
                0
            } else if option_name_eq(name, "SubDJoinedEdges") {
                result.crease = if option_name_eq(value, "Smooth") {
                    false
                } else if option_name_eq(value, "Crease") {
                    true
                } else {
                    return Err(CommandError::Usage(usage));
                };
                1
            } else {
                return Err(CommandError::Usage(usage));
            };
            if std::mem::replace(&mut seen[field], true) {
                return Err(CommandError::Usage(usage));
            }
            index += count;
        }
        Ok(result)
    }

    fn execute(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = self.parse(arguments)?;
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
        let meshes = sources
            .iter()
            .all(|o| matches!(o.geometry(), Geometry::Mesh(_)));
        let surfaces = sources.iter().all(|o| breps::accepts(o.geometry()));
        let subds = sources
            .iter()
            .all(|o| matches!(o.geometry(), Geometry::SubD(_)));
        let plan = if meshes {
            meshes::stage(&sources, document.tolerance(), options.disjoint)?
        } else if surfaces {
            breps::stage(&sources, document.tolerance(), postselected)?
        } else if subds {
            subds::stage(&sources, document.tolerance(), options.crease, postselected)?
        } else {
            curves::stage(
                &sources,
                document.tolerance(),
                postselected,
                self.copy_inputs,
            )?
        };
        if plan.copies.is_empty() && (sources.len() == 1 || postselected) {
            return Err(CommandError::NothingJoined);
        }
        let keep_sources = self.copy_inputs && (meshes || surfaces || subds || plan.closed_on_pick);
        if subds && postselected {
            let picks = sources.iter().map(|o| o.id()).collect::<Vec<_>>();
            document.release_command_selection_on_history_replay(picks)?;
        }
        let outputs = document.copy_object_pieces_into_source_groups(plan.copies)?;
        document.select_objects_direct(plan.release, SelectionMode::Remove)?;
        if postselected && keep_sources {
            // Rejected/unconnected picks are not retained when the copied
            // chain completes; only the participating originals stay selected.
            document
                .select_objects_direct(plan.consumed.iter().copied(), SelectionMode::Replace)?;
        }
        if !self.copy_inputs {
            document.delete_objects(plan.consumed)?;
        }
        if postselected {
            if !keep_sources {
                document.clear_selection();
            }
        } else {
            // Keep selected originals/singletons without expanding their groups.
            document.select_objects_direct(outputs, SelectionMode::Add)?;
        }
        self.disjoint.set(options.disjoint);
        self.crease.set(options.crease);
        Ok(plan.description)
    }
}

struct JoinOptions {
    disjoint: bool,
    crease: bool,
}

/// All geometry is validated before the shared, transactional document edit.
struct JoinPlan {
    copies: Vec<(ObjectId, Geometry)>,
    consumed: Vec<ObjectId>,
    description: String,
    closed_on_pick: bool,
    release: Vec<ObjectId>,
}

#[cfg(feature = "native-smlib")]
pub(crate) use breps::join_breps;
