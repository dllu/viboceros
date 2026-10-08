//! BooleanSplit document policy over one exact original-operand arrangement.
use super::*;
use std::borrow::Cow;

const USAGE: &str = "BooleanSplit [DeleteInput=Yes|No] FirstSet=ids SecondSet=ids";
#[cfg(test)]
mod tests;

pub(super) struct BooleanSplitCommand {
    delete_input: remembered::Remembered<bool>,
}
impl Default for BooleanSplitCommand {
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
impl BooleanSplitCommand {
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
        self.delete_input.set(options.delete_input);
        let (Some(first), Some(second)) = (options.first, options.second) else {
            return Err(CommandError::Usage(USAGE));
        };
        let mut seen = BTreeSet::new();
        let ids = first
            .iter()
            .chain(&second)
            .copied()
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();
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
                let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
                if !eligible.contains(&id) {
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
                _ => unreachable!("surface filter"),
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let refs = breps.iter().map(Cow::as_ref).collect::<Vec<_>>();
        let closed = refs
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_solid())
            .collect::<Vec<_>>();
        let (_, contacts) = boolean_solids::interactions(
            &closed.iter().map(|(_, b)| **b).collect::<Vec<_>>(),
            doc.tolerance(),
            false,
        )?;
        let interactions = contacts
            .into_iter()
            .map(|[a, b]| [closed[a].0, closed[b].0])
            .collect::<Vec<_>>();
        let surfaces = refs.iter().any(|b| !b.is_solid());
        let indices = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect::<BTreeMap<_, _>>();
        let mut copies = Vec::new();
        let mut changed = Vec::new();
        for &target in &first {
            let index = indices[&target];
            let cutters = second
                .iter()
                .filter_map(|id| {
                    let j = indices[id];
                    (j != index
                        && (!refs[index].is_solid()
                            || !refs[j].is_solid()
                            || interactions.contains(&[index.min(j), index.max(j)])))
                    .then_some(refs[j])
                })
                .collect::<Vec<_>>();
            if cutters.is_empty() {
                continue;
            }
            let pieces = if !refs[index].is_solid() {
                viboceros_geometry::split_open_polyhedral_brep(
                    refs[index],
                    &cutters,
                    doc.tolerance(),
                )?
                .into_iter()
                .map(|p| (p.brep, p.face_sources, vec![1]))
                .collect::<Vec<_>>()
            } else if surfaces {
                let pieces = viboceros_geometry::split_polyhedral_brep_with_surfaces(
                    refs[index],
                    &cutters,
                    doc.tolerance(),
                )?;
                if !pieces
                    .iter()
                    .any(|p| p.cut_sides.iter().any(Option::is_some))
                {
                    continue;
                }
                pieces
                    .into_iter()
                    .map(|p| (p.brep, p.face_sources, p.branch_component_counts))
                    .collect::<Vec<_>>()
            } else {
                viboceros_geometry::split_polyhedral_brep(refs[index], &cutters, doc.tolerance())?
                    .into_iter()
                    .map(|p| (p.brep, p.face_sources, p.branch_component_counts))
                    .collect::<Vec<_>>()
            };
            if pieces.len() <= 1 {
                continue;
            }
            changed.push(target);
            for (brep, sources, branches) in pieces {
                let text = if branches.iter().all(|&n| n == 1) {
                    objects[index].geometry_user_text().clone()
                } else {
                    BTreeMap::new()
                };
                let brep = boolean_solids::merged(brep, &sources, doc.tolerance())?;
                copies.push((target, Geometry::Brep(brep), text));
            }
        }
        if changed.is_empty() {
            return Err(CommandError::NothingSplit);
        }
        doc.clear_selection();
        doc.release_command_selection_on_history_replay(ids)?;
        let outputs = doc.copy_object_pieces_with_metadata_into_source_groups(copies)?;
        if options.delete_input {
            doc.delete_objects(changed)?;
        }
        if !post {
            let selected = doc
                .selected_object_ids()
                .chain(outputs.iter().copied())
                .collect::<Vec<_>>();
            doc.select_command_results(selected)?;
        }
        Ok(format!(
            "Boolean split created {} polysurface(s); retained cutters",
            outputs.len()
        ))
    }
}
impl Command for BooleanSplitCommand {
    fn name(&self) -> &'static str {
        "BooleanSplit"
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
            command: "BooleanSplit",
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
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.delete_input.set(self.parse(args)?.delete_input);
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
    fn cleanup_failed_selection(&self, doc: &mut Document, error: &CommandError, _post: bool) {
        if matches!(error, CommandError::NothingSplit) {
            doc.clear_selection();
        }
    }
}
