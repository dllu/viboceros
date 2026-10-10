//! Native subtraction policies over exact polyhedral construction.
use super::*;
use std::borrow::Cow;

#[cfg(feature = "native-smlib")]
mod native;
#[cfg(test)]
mod tests;
const USAGE: &str =
    "BooleanDifference [DeleteInput=Yes|No] [DeleteCutters=Yes|No] FirstSet=ids SecondSet=ids";
#[derive(Clone, Copy)]
struct Options {
    delete_input: bool,
    delete_cutters: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            delete_input: true,
            delete_cutters: true,
        }
    }
}
#[derive(Default)]
pub(super) struct BooleanDifferenceCommand {
    options: remembered::Remembered<Options>,
}
struct Arguments {
    options: Options,
    first: Option<Vec<ObjectId>>,
    second: Option<Vec<ObjectId>>,
}
impl BooleanDifferenceCommand {
    fn parse(&self, args: &[&str]) -> Result<Arguments, CommandError> {
        let mut result = Arguments {
            options: self.options.get(),
            first: None,
            second: None,
        };
        let (mut index, mut input_seen, mut cutters_seen) = (0, false, false);
        while index < args.len() {
            let (name, value, consumed) = orient_option(args, index, USAGE)?;
            if option_name_eq(name, "DeleteInput") && !input_seen {
                result.options.delete_input =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                input_seen = true;
            } else if option_name_eq(name, "DeleteCutters") && !cutters_seen {
                result.options.delete_cutters =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                cutters_seen = true;
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
        let arguments = self.parse(args)?;
        self.options.set(arguments.options);
        let (Some(first), Some(second)) = (arguments.first, arguments.second) else {
            return Err(CommandError::Usage(USAGE));
        };
        let ids = first.iter().chain(&second).copied().collect::<Vec<_>>();
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(CommandError::Usage(USAGE));
        }
        if ids.len() > 128 {
            return Err(GeometryError::BrepBooleanWorkLimit.into());
        }
        let eligible = doc
            .selectable_objects()
            .chain(doc.selected_objects())
            .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        let objects = ids
            .iter()
            .map(|&id| {
                let o = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
                if !eligible.contains(&id) {
                    return Err(CommandError::Usage(USAGE));
                }
                Ok(o)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let breps = objects
            .iter()
            .map(|o| match o.geometry() {
                Geometry::Brep(b) => Ok(Cow::Borrowed(b)),
                Geometry::NurbsSurface(s) => {
                    Brep::try_surface_face(s.clone(), doc.tolerance()).map(Cow::Owned)
                }
                _ => unreachable!("surface filter"),
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let refs = breps.iter().map(Cow::as_ref).collect::<Vec<_>>();
        let prepared = (|| -> Result<_, CommandError> {
            let (kernel, interactions) =
                boolean_solids::interactions(&refs, doc.tolerance(), true)?;
            if !interactions
                .iter()
                .any(|p| p[0] < first.len() && p[1] >= first.len())
            {
                return Err(CommandError::NothingSubtracted);
            }
            let mut copies = Vec::new();
            for (target, &id) in first.iter().enumerate() {
                let cutters = interactions
                    .iter()
                    .filter(|p| p[0] == target && p[1] >= first.len())
                    .map(|p| refs[p[1]])
                    .collect::<Vec<_>>();
                if cutters.is_empty() {
                    copies.push((
                        id,
                        Geometry::Brep(refs[target].clone()),
                        objects[target].geometry_user_text().clone(),
                    ));
                    continue;
                }
                let local = std::iter::once(refs[target])
                    .chain(cutters.iter().copied())
                    .collect::<Vec<_>>();
                let pairs = (1..local.len()).map(|i| [0, i]).collect::<Vec<_>>();
                let active = boolean_solids::active_faces(&local, &pairs, doc.tolerance(), true)?;
                let mut shells = Vec::new();
                for piece in kernel.difference(refs[target], &cutters, doc.tolerance())? {
                    shells.extend(boolean_solids::participating_shells(
                        piece.brep,
                        &piece.face_sources,
                        &active,
                        doc.tolerance(),
                    )?);
                }
                let text = if shells.len() == 1 {
                    objects[target].geometry_user_text().clone()
                } else {
                    BTreeMap::new()
                };
                for (brep, sources) in shells {
                    let b = boolean_solids::merged(brep, &sources, doc.tolerance())?;
                    copies.push((id, Geometry::Brep(b), text.clone()));
                }
            }
            Ok(copies)
        })();
        #[cfg(feature = "native-smlib")]
        let copies = match prepared {
            Err(CommandError::Geometry(
                GeometryError::UnsupportedConvexBrepBoolean { .. }
                | GeometryError::UnsupportedPolyhedralBrepBoolean { .. },
            )) => native::difference(&refs, &objects, &first, doc.tolerance())?,
            result => result?,
        };
        #[cfg(not(feature = "native-smlib"))]
        let copies = prepared?;
        // Successful full removal has no copies; it still consumes accepted inputs.
        doc.select_command_results(ids.iter().copied())?;
        doc.release_command_selection_on_history_replay(if post {
            ids.clone()
        } else {
            second.clone()
        })?;
        let outputs = doc.copy_object_pieces_with_metadata_into_source_groups(copies)?;
        if arguments.options.delete_input {
            let deleted = if arguments.options.delete_cutters {
                ids
            } else {
                first
            };
            doc.delete_objects(deleted)?;
        }
        if !post {
            let selected = doc
                .selected_object_ids()
                .chain(outputs.iter().copied())
                .collect::<Vec<_>>();
            doc.select_command_results(selected)?;
        }
        Ok(format!(
            "Boolean difference created {} polysurface(s)",
            outputs.len()
        ))
    }
}
impl Command for BooleanDifferenceCommand {
    fn name(&self) -> &'static str {
        "BooleanDifference"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let a = self.parse(args)?;
        if a.first.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "BooleanDifference",
            filter: ObjectSelectionFilter::SurfaceComponents,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![
                BooleanSelectionOption {
                    name: "DeleteInput",
                    value: a.options.delete_input,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "DeleteCutters",
                    value: a.options.delete_cutters,
                    aliases: &[],
                },
            ],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        let a = self.parse(args)?;
        self.options.set(a.options);
        Ok(())
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
