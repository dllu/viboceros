//! Polyhedral BooleanUnion command; exact construction stays in geometry.
use super::*;

#[cfg(test)]
pub(crate) mod tests;

const USAGE: &str = "BooleanUnion [DeleteInput=Yes|No] [MergeCoplanarFaces=Yes|No]";

#[derive(Clone, Copy)]
struct Options {
    delete_input: bool,
    merge_coplanar: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delete_input: true,
            merge_coplanar: true,
        }
    }
}

#[derive(Default)]
pub(super) struct BooleanUnionCommand {
    options: remembered::Remembered<Options>,
}

impl BooleanUnionCommand {
    fn parse(&self, arguments: &[&str]) -> Result<Options, CommandError> {
        let mut options = self.options.get();
        let (mut delete_seen, mut merge_seen, mut index) = (false, false, 0);
        while index < arguments.len() {
            let (name, value, consumed) = orient_option(arguments, index, USAGE)?;
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "DeleteInput") && !delete_seen {
                options.delete_input = value;
                delete_seen = true;
            } else if option_name_eq(name, "MergeCoplanarFaces") && !merge_seen {
                options.merge_coplanar = value;
                merge_seen = true;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            index += consumed;
        }
        Ok(options)
    }

    fn execute(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = self.parse(arguments)?;
        // Accepted command options survive failure/cancellation and model history.
        self.options.set(options);
        let selected = if postselected {
            document.selected_objects().collect::<Vec<_>>()
        } else {
            document
                .objects()
                .filter(|o| document.is_selected(o.id()))
                .collect()
        };
        let excluded = selected
            .iter()
            .filter(|o| !ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let sources = selected
            .into_iter()
            .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
            .collect::<Vec<_>>();
        if sources.len() < 2 {
            return Err(CommandError::BooleanUnionRequiresTwoObjects);
        }
        let breps = sources
            .iter()
            .map(|o| match o.geometry() {
                Geometry::Brep(brep) => Ok(brep.clone()),
                Geometry::NurbsSurface(surface) => {
                    Brep::try_surface_face(surface.clone(), document.tolerance())
                }
                _ => unreachable!("surface selection filter"),
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let refs = breps.iter().collect::<Vec<_>>();
        let prepared = (|| -> Result<_, CommandError> {
            let (kernel, interactions) =
                boolean_solids::interactions(&refs, document.tolerance(), false)?;
            if interactions.is_empty() {
                return Err(CommandError::NothingUnioned);
            }
            let components = kernel.union(&refs, document.tolerance())?;
            let active =
                boolean_solids::active_faces(&refs, &interactions, document.tolerance(), false)?;
            let mut copies = Vec::new();
            let mut consumed = Vec::new();
            for component in components {
                if !interactions
                    .iter()
                    .any(|pair| pair.iter().all(|i| component.source_indices.contains(i)))
                {
                    continue;
                }
                let owner = component.boundary_source_indices[0];
                let geometry_owner = *component
                    .boundary_source_indices
                    .last()
                    .expect("nonempty boundary");
                let shells = boolean_solids::participating_shells(
                    component.brep,
                    &component.face_sources,
                    &active,
                    document.tolerance(),
                )?;
                let text = if shells.len() == 1 {
                    sources[geometry_owner].geometry_user_text().clone()
                } else {
                    BTreeMap::new()
                };
                for (brep, sources_of_faces) in shells {
                    let brep = if options.merge_coplanar {
                        brep.try_merge_coplanar_polygon_faces_in_groups(
                            &vec![0; brep.faces().len()],
                            document.tolerance(),
                        )?
                        .unwrap_or(brep)
                        .try_merge_all_edges(0., document.tolerance())?
                    } else {
                        boolean_solids::merged(brep, &sources_of_faces, document.tolerance())?
                    };
                    copies.push((sources[owner].id(), Geometry::Brep(brep), text.clone()));
                }
                consumed.extend(component.source_indices.iter().map(|&i| sources[i].id()));
            }
            if copies.is_empty() {
                return Err(CommandError::NothingUnioned);
            }
            Ok((copies, consumed))
        })();
        #[cfg(feature = "native-smlib")]
        let (copies, consumed) = match prepared {
            Err(CommandError::Geometry(
                GeometryError::UnsupportedConvexBrepBoolean { .. }
                | GeometryError::UnsupportedPolyhedralBrepBoolean { .. },
            )) => {
                let result = boolean_solids::native::union(
                    &refs,
                    &sources,
                    options.merge_coplanar,
                    document.tolerance(),
                )?;
                (result.copies, result.consumed)
            }
            result => result?,
        };
        #[cfg(not(feature = "native-smlib"))]
        let (copies, consumed) = prepared?;
        let all_sources = sources.iter().map(|o| o.id()).collect::<Vec<_>>();
        // Construction, merging and metadata staging have finished before edits.
        let outputs = document.copy_object_pieces_with_metadata_into_source_groups(copies)?;
        document.select_objects_direct(excluded, SelectionMode::Remove)?;
        if postselected {
            document.release_command_selection_on_history_replay(all_sources)?;
        }
        if options.delete_input {
            document.delete_objects(consumed)?;
        }
        if !postselected {
            // Source-group memberships must not expand the result selection.
            let selection = document
                .selected_object_ids()
                .chain(outputs.iter().copied())
                .collect::<Vec<_>>();
            document.select_command_results(selection)?;
        }
        Ok(format!(
            "Boolean union created {} polysurface(s)",
            outputs.len()
        ))
    }
}

impl Command for BooleanUnionCommand {
    fn name(&self) -> &'static str {
        "BooleanUnion"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "BooleanUnion",
            filter: ObjectSelectionFilter::SurfaceComponents,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![
                BooleanSelectionOption {
                    name: "DeleteInput",
                    value: options.delete_input,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "MergeCoplanarFaces",
                    value: options.merge_coplanar,
                    aliases: &[],
                },
            ],
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.options.set(self.parse(arguments)?);
        Ok(())
    }

    fn cancel_empty_object_selection(&self) -> bool {
        true
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        postselected: bool,
    ) {
        if postselected && matches!(error, CommandError::BooleanUnionRequiresTwoObjects) {
            document.clear_selection();
        }
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
}
