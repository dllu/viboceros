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
    command: &'static str,
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
            command: USAGE,
        })
    }
    /// Stage one original face pair or the connected planar region of a seed.
    pub fn prepare_selected(
        doc: &Document,
        id: ObjectId,
        faces: &[usize],
        region: bool,
    ) -> Result<Self, CommandError> {
        let object = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(CommandError::NoObjectsSelected)?;
        let face_count = match object.geometry() {
            Geometry::Brep(b) => b.faces().len(),
            Geometry::Mesh(m) => m.face_count(),
            Geometry::NurbsSurface(_) => 1,
            _ => return Err(CommandError::UnsupportedCoplanarMergeGeometry),
        };
        if faces.len() != if region { 1 } else { 2 }
            || faces.iter().any(|&f| f >= face_count)
            || (!region && faces[0] == faces[1])
        {
            return Err(CommandError::Usage(
                "Select two distinct original faces or one region seed",
            ));
        }
        let merged = match object.geometry() {
            Geometry::NurbsSurface(_) => None,
            Geometry::Brep(b) if b.faces().len() == 1 => None,
            Geometry::Brep(b) => merge_brep(b, doc.tolerance(), Some((faces, region)))?
                .map(|(b, n)| (Geometry::Brep(b), n)),
            Geometry::Mesh(m) => {
                let (m, n) = m.merge_coplanar_faces_at(faces, region, doc.tolerance())?;
                (n > 0).then_some((Geometry::Mesh(m), n))
            }
            _ => unreachable!(),
        };
        let (replacements, removed) = if let Some((g, n)) = merged {
            (vec![(id, g)], n)
        } else {
            (vec![], 0)
        };
        Ok(Self {
            sources: vec![(id, object.geometry_snapshot().clone())],
            replacements,
            removed,
            tolerance: doc.tolerance(),
            command: "MergeCoplanarFace",
        })
    }
    pub fn commit(&self, doc: &mut Document) -> Result<(usize, usize), CommandError> {
        run_command_transaction(doc, self.command, |doc| self.apply(doc))
    }
    pub(super) fn apply(&self, doc: &mut Document) -> Result<(usize, usize), CommandError> {
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
    merge_brep(source, tolerance, None)
}
fn merge_brep(
    source: &Brep,
    tolerance: Tolerance,
    selection: Option<(&[usize], bool)>,
) -> Result<Option<(Brep, usize)>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let controls = source.faces().iter().fold(0usize, |n, f| {
            n.saturating_add(f.surface().control_points().len())
        });
        if source.faces().len() > 362 || controls > 2_000_000 {
            return Err(CommandError::Usage(
                "Coplanar face merging supports at most 362 faces and 2,000,000 surface controls per B-rep",
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
        let result = if let Some((faces, region)) = selection {
            viboceros_smlib::Shape::merge_brep_coplanar_subset(&local, faces, region, tolerance)?
        } else {
            viboceros_smlib::Shape::from_brep(&local, tolerance)?.merge_coplanar_faces(tolerance)?
        };
        match result {
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
        let _ = (source, tolerance, selection);
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
