//! In-place planar face merging, staged independently of document mutation.
use super::*;
use viboceros_document::GeometrySnapshot;
const USAGE: &str = "MergeAllCoplanarFaces";
pub(super) struct MergeAllCoplanarFacesCommand;
#[derive(Clone, Debug)]
pub struct CoplanarMergeSelection {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
    tolerance: Tolerance,
    removed: usize,
}
impl CoplanarMergeSelection {
    pub fn prepare(doc: &Document) -> Result<Self, CommandError> {
        let mut sources = Vec::new();
        let mut replacements = Vec::new();
        let mut removed = 0;
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                return Err(CommandError::Usage(USAGE));
            }
            let merged = match object.geometry() {
                Geometry::Brep(b) if b.faces().len() > 1 => {
                    merge(b, doc.tolerance())?.map(|(b, n)| (Geometry::Brep(b), n))
                }
                Geometry::Brep(_) | Geometry::NurbsSurface(_) => None,
                Geometry::Mesh(m) => {
                    let (result, n) = m.merge_coplanar_faces(doc.tolerance())?;
                    (n > 0).then_some((Geometry::Mesh(result), n))
                }
                _ => return Err(CommandError::UnsupportedCoplanarMergeGeometry),
            };
            sources.push((object.id(), object.geometry_snapshot().clone()));
            if let Some((geometry, n)) = merged {
                removed += n;
                replacements.push((object.id(), geometry));
            }
        }
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        Ok(Self {
            sources,
            replacements,
            tolerance: doc.tolerance(),
            removed,
        })
    }
    pub fn commit(&self, doc: &mut Document) -> Result<(usize, usize), CommandError> {
        run_command_transaction(doc, USAGE, |doc| self.apply(doc))
    }
    fn apply(&self, doc: &mut Document) -> Result<(usize, usize), CommandError> {
        if doc.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, snapshot)| {
                !doc.is_object_selectable(*id)
                    || !doc
                        .object(*id)
                        .is_some_and(|o| o.geometry_snapshot() == snapshot)
            })
        {
            return Err(CommandError::Usage(
                "Coplanar merge source or tolerance changed; select objects again",
            ));
        }
        let changed = self.replacements.len();
        doc.replace_object_geometries(self.replacements.clone())?;
        Ok((self.removed, changed))
    }
}
fn merge(source: &Brep, tolerance: Tolerance) -> Result<Option<(Brep, usize)>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let controls = source.faces().iter().fold(0usize, |n, f| {
            n.saturating_add(f.surface().control_points().len())
        });
        if source.faces().len() > 362 || controls > 2_000_000 {
            return Err(CommandError::Usage(
                "MergeAllCoplanarFaces supports at most 362 faces and 2,000,000 surface controls per B-rep",
            ));
        }
        let origin = source
            .vertices()
            .first()
            .map(|v| v.point())
            .unwrap_or_else(|| source.bounds().min());
        let zero = Point3::try_new(0., 0., 0.)?;
        let local = source.transformed(
            AffineTransform3::from_translation(origin.vector_to(zero)?),
            tolerance,
        )?;
        let shape = viboceros_smlib::Shape::from_brep(&local, tolerance)?;
        match shape.merge_coplanar_faces(tolerance)? {
            None => Ok(None),
            Some((merged, count)) => Ok(Some((
                merged.to_brep(tolerance)?.transformed(
                    AffineTransform3::from_translation(zero.vector_to(origin)?),
                    tolerance,
                )?,
                count,
            ))),
        }
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (source, tolerance);
        Err(CommandError::CoplanarMergeRequiresNativeKernel)
    }
}
impl Command for MergeAllCoplanarFacesCommand {
    fn name(&self) -> &'static str {
        USAGE
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, USAGE)?;
        Ok(Some(ObjectSelectionPrompt {
            command: USAGE,
            filter: ObjectSelectionFilter::CoplanarFaces,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        require_consumed(args, 0, USAGE)?;
        let (removed, objects) = CoplanarMergeSelection::prepare(doc)?.apply(doc)?;
        Ok(format!(
            "Merged {removed} coplanar face(s) in {objects} object(s)"
        ))
    }
}
#[cfg(test)]
mod tests;
