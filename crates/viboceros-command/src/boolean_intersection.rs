//! BooleanIntersection common-set and two-set document policies.
use super::*;
use std::borrow::Cow;

#[cfg(test)]
mod tests;
const USAGE: &str = "BooleanIntersection [DeleteInput=Yes|No] [FirstSet=ids] [SecondSet=ids]";

pub(super) struct BooleanIntersectionCommand {
    delete_input: remembered::Remembered<bool>,
}
impl Default for BooleanIntersectionCommand {
    fn default() -> Self {
        Self {
            delete_input: remembered::Remembered::new(true),
        }
    }
}
struct Arguments {
    delete_input: bool,
    first: Option<Vec<ObjectId>>,
    second: Option<Vec<ObjectId>>,
}
impl BooleanIntersectionCommand {
    fn parse(&self, args: &[&str]) -> Result<Arguments, CommandError> {
        let mut result = Arguments {
            delete_input: self.delete_input.get(),
            first: None,
            second: None,
        };
        let (mut index, mut delete_seen) = (0, false);
        while index < args.len() {
            let (name, value, consumed) = orient_option(args, index, USAGE)?;
            if option_name_eq(name, "DeleteInput") && !delete_seen {
                result.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                delete_seen = true;
            } else if option_name_eq(name, "FirstSet") && result.first.is_none() {
                result.first = Some(boolean_solids::parse_ids(value, USAGE)?);
            } else if option_name_eq(name, "SecondSet") && result.second.is_none() {
                result.second = Some(boolean_solids::parse_ids(value, USAGE)?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            index += consumed;
        }
        if result.second.is_some() && result.first.is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(result)
    }
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let options = self.parse(args)?;
        let first = options.first.unwrap_or_else(|| {
            if post {
                doc.selected_objects()
                    .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
                    .map(|o| o.id())
                    .collect()
            } else {
                doc.objects()
                    .filter(|o| {
                        doc.is_selected(o.id())
                            && ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
                    })
                    .map(|o| o.id())
                    .collect()
            }
        });
        let second = options.second.unwrap_or_default();
        let common = second.is_empty();
        if first.is_empty() || (common && first.len() < 2) {
            return Err(CommandError::BooleanIntersectionRequiresObjects);
        }
        let ids = first.iter().chain(&second).copied().collect::<Vec<_>>();
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(CommandError::Usage(USAGE));
        }
        if ids.len() > 128 {
            return Err(GeometryError::BrepBooleanWorkLimit.into());
        }
        let selectable = doc
            .selectable_objects()
            .chain(doc.selected_objects())
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        let objects = ids
            .iter()
            .map(|&id| {
                let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
                if !selectable.contains(&id) {
                    return Err(CommandError::Usage(USAGE));
                }
                Ok(object)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let breps = objects
            .iter()
            .map(|o| match o.geometry() {
                Geometry::Brep(b) => Ok(Cow::Borrowed(b)),
                Geometry::NurbsSurface(s) => {
                    Brep::try_surface_face(s.clone(), doc.tolerance()).map(Cow::Owned)
                }
                _ => Err(GeometryError::UnsupportedConvexBrepBoolean {
                    context: "surface or polysurface inputs are required",
                }),
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let refs = breps.iter().map(Cow::as_ref).collect::<Vec<_>>();
        let compound_sets = refs
            .iter()
            .any(|b| b.edge_connected_face_components().len() > 1);
        let (kernel, interactions) = if compound_sets {
            // The compound plan certifies all inputs and tests its oriented
            // shells. Avoid constructing an unused material-only arrangement.
            (boolean_solids::Kernel::Polyhedral, Vec::new())
        } else {
            boolean_solids::interactions(&refs, doc.tolerance(), false)?
        };
        if !compound_sets
            && !interactions
                .iter()
                .any(|p| common || (p[0] < first.len() && p[1] >= first.len()))
        {
            return Err(CommandError::NothingIntersected);
        }
        let mut copies = Vec::new();
        if let Some(results) = boolean_solids::compound_intersection(
            &refs,
            doc.tolerance(),
            common,
            first.len(),
            kernel,
        )? {
            for result in results {
                let brep = if common {
                    let b = result.component.brep;
                    // Common intersection merges coplanar boundary faces across
                    // original owners. Two-set pairs retain their source seams.
                    boolean_solids::finish_boundary(
                        b.try_merge_coplanar_polygon_faces_in_groups(
                            &vec![0; b.faces().len()],
                            doc.tolerance(),
                        )?
                        .unwrap_or(b),
                        doc.tolerance(),
                    )?
                } else {
                    boolean_solids::merged(
                        result.component.brep,
                        &result.component.face_sources,
                        doc.tolerance(),
                    )?
                };
                let text = result
                    .geometry_owner
                    .map(|i| objects[i].geometry_user_text().clone())
                    .unwrap_or_default();
                copies.push((ids[result.owner], Geometry::Brep(brep), text));
            }
        } else if common {
            let results = kernel.common(&refs, doc.tolerance())?;
            let text = if results.len() == 1 {
                objects[objects.len() - 1].geometry_user_text().clone()
            } else {
                BTreeMap::new()
            };
            for result in results {
                let brep =
                    boolean_solids::merged(result.brep, &result.face_sources, doc.tolerance())?;
                copies.push((ids[0], Geometry::Brep(brep), text.clone()));
            }
        } else {
            let results =
                kernel.sets(&refs[..first.len()], &refs[first.len()..], doc.tolerance())?;
            let mut counts = BTreeMap::new();
            for result in &results {
                *counts.entry(result.maximal_pairs[0][0]).or_insert(0usize) += 1;
            }
            for result in results {
                let owner = result.maximal_pairs[0][0];
                let geometry_owner = result.maximal_pairs.last().unwrap()[0];
                let brep =
                    boolean_solids::merged(result.brep, &result.face_sources, doc.tolerance())?;
                copies.push((
                    ids[owner],
                    Geometry::Brep(brep),
                    if counts[&owner] == 1 {
                        objects[geometry_owner].geometry_user_text().clone()
                    } else {
                        BTreeMap::new()
                    },
                ));
            }
        }
        if copies.is_empty() {
            return Err(CommandError::NothingIntersected);
        }
        // First-set picks stay selected through the second phase. Geometry and
        // metadata are completely staged before this transactional model edit.
        doc.select_command_results(ids.iter().copied())?;
        doc.release_command_selection_on_history_replay(if post {
            ids.clone()
        } else {
            second.clone()
        })?;
        let outputs = doc.copy_object_pieces_with_metadata_into_source_groups(copies)?;
        if options.delete_input {
            doc.delete_objects(ids)?;
        }
        if !post && !common {
            let selection = doc
                .selected_object_ids()
                .chain(outputs.iter().copied())
                .collect::<Vec<_>>();
            doc.select_command_results(selection)?;
        }
        self.delete_input.set(options.delete_input);
        Ok(format!(
            "Boolean intersection created {} polysurface(s)",
            outputs.len()
        ))
    }
}
impl Command for BooleanIntersectionCommand {
    fn name(&self) -> &'static str {
        "BooleanIntersection"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(args)?;
        if options.first.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "BooleanIntersection",
            filter: ObjectSelectionFilter::SurfaceComponents,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: options.delete_input,
                aliases: &[],
            }],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, true)
    }
}
