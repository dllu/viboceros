//! Object identities and attributes for validated Inset geometry.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Replace,
    Retain,
    Recreate,
}
#[derive(Clone, Debug)]
pub(super) struct Output {
    pub source: ObjectId,
    pub geometries: Vec<Geometry>,
    pub action: Action,
}
impl Plan {
    pub(super) fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::InsetStale);
        }
        doc.replace_object_geometries(
            self.outputs
                .iter()
                .filter(|o| o.action == Action::Replace)
                .map(|o| (o.source, o.geometries[0].clone())),
        )?;
        doc.delete_objects(
            self.outputs
                .iter()
                .filter(|o| o.action == Action::Recreate)
                .map(|o| o.source),
        )?;
        for output in &self.outputs {
            if output.action == Action::Replace {
                continue;
            }
            for geometry in &output.geometries {
                // Native curve and separate-surface outputs use new default
                // attributes on the current layer and start unselected.
                doc.add_geometry(geometry.clone())?;
            }
        }
        Ok(format!(
            "Inset {} face(s) on {} object(s)",
            self.faces.len(),
            self.witness.len()
        ))
    }
}
