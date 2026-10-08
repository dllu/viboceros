//! Finite coplanar surface Booleans; document policy follows public commands.
use super::*;
use std::borrow::Cow;
use viboceros_geometry::{BrepBooleanOperation, boolean_planar_breps};
#[cfg(test)]
mod tests;

pub(super) struct PlanarBooleanCommand(pub(super) BrepBooleanOperation);
impl Command for PlanarBooleanCommand {
    fn name(&self) -> &'static str {
        match self.0 {
            BrepBooleanOperation::Union => "PlanarUnion",
            BrepBooleanOperation::Difference => "PlanarDifference",
            BrepBooleanOperation::Intersection => "PlanarIntersection",
        }
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
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if parse(args)?.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}
impl PlanarBooleanCommand {
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let explicit = parse(args)?;
        let ids = explicit.unwrap_or_else(|| {
            doc.selected_objects()
                .filter(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
                .map(|o| o.id())
                .collect()
        });
        if ids.len() < 2
            || (self.0 != BrepBooleanOperation::Union && ids.len() != 2)
            || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(CommandError::Usage(
                "Select distinct coplanar surfaces: at least two for Union, exactly two otherwise",
            ));
        }
        let surfaces = ids
            .iter()
            .map(|&id| {
                let o = doc
                    .object(id)
                    .filter(|o| {
                        doc.is_object_selectable(id)
                            && ObjectSelectionFilter::Surfaces.accepts_object(o)
                    })
                    .ok_or(DocumentError::ObjectNotFound(id))?;
                match o.geometry() {
                    Geometry::Brep(b) => Ok(Cow::Borrowed(b)),
                    Geometry::NurbsSurface(s) => Ok(Cow::Owned(Brep::try_surface_face(
                        s.clone(),
                        doc.tolerance(),
                    )?)),
                    _ => unreachable!(),
                }
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let refs = surfaces.iter().map(Cow::as_ref).collect::<Vec<_>>();
        let outputs = boolean_planar_breps(&refs, self.0, doc.tolerance())?;
        let mut pieces = Vec::new();
        for b in outputs {
            let b = if b
                .edges()
                .iter()
                .all(|e| e.curve().degree() == 1 && e.curve().control_points().len() == 2)
            {
                let groups = vec![0; b.faces().len()];
                boolean_solids::finish_boundary(
                    b.try_merge_coplanar_polygon_faces_in_groups(&groups, doc.tolerance())?
                        .unwrap_or(b),
                    doc.tolerance(),
                )?
            } else {
                b
            };
            pieces.push(boolean_two::Piece {
                brep: b,
                owner: 0,
                retain_geometry_user_text: false,
            });
        }
        let preserve_preselected = !post
            && self.0 != BrepBooleanOperation::Difference
            && ids.iter().any(|&id| doc.is_selected(id));
        if !preserve_preselected {
            doc.release_command_selection_on_history_replay(ids.iter().copied())?;
            doc.clear_selection();
        }
        if self.0 == BrepBooleanOperation::Intersection && !pieces.is_empty() {
            let id = ids[0];
            let keys = doc
                .object(id)
                .unwrap()
                .geometry_user_text()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            doc.replace_object_geometries([(id, Geometry::Brep(pieces[0].brep.clone()))])?;
            for key in keys {
                doc.set_object_geometry_user_text([id], &key, None)?;
            }
            for p in pieces.iter().skip(1) {
                doc.copy_object_pieces_with_metadata_into_source_groups([(
                    id,
                    Geometry::Brep(p.brep.clone()),
                    BTreeMap::new(),
                )])?;
            }
            doc.delete_objects([ids[1]])?;
        } else {
            for p in &pieces {
                doc.add_geometry(Geometry::Brep(p.brep.clone()))?;
            }
            doc.delete_objects(ids.iter().copied())?;
        }
        if preserve_preselected
            && self.0 == BrepBooleanOperation::Intersection
            && !pieces.is_empty()
        {
            doc.release_transform_selection_on_history_replay([ids[0]])?;
        }
        doc.clear_selection();

        Ok(format!(
            "{} created {} planar surface(s)",
            self.name(),
            pieces.len()
        ))
    }
}

fn parse(args: &[&str]) -> Result<Option<Vec<ObjectId>>, CommandError> {
    match args {
        [] => Ok(None),
        [option] => {
            let (name, value) = option
                .split_once('=')
                .ok_or(CommandError::Usage("Sources=id,id"))?;
            if !option_name_eq(name, "Sources") {
                return Err(CommandError::Usage("Sources=id,id"));
            }
            Ok(Some(boolean_solids::parse_ids(value, "Sources=id,id")?))
        }
        _ => Err(CommandError::Usage("Sources=id,id")),
    }
}
