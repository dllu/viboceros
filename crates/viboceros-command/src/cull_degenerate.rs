//! Stage native cleanup for every target before one document transaction.
use super::*;
use viboceros_document::GeometrySnapshot;
const USAGE: &str = "CullDegenerateMeshFaces";
pub(super) struct CullDegenerateMeshFacesCommand;
#[derive(Clone, Debug)]
pub struct DegenerateFaceSelection {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
    deleted: Vec<ObjectId>,
    removed_faces: usize,
    repaired_faces: usize,
    removed_vertices: usize,
}
impl DegenerateFaceSelection {
    pub fn prepare(doc: &Document) -> Result<Self, CommandError> {
        let mut result = Self {
            sources: vec![],
            replacements: vec![],
            deleted: vec![],
            removed_faces: 0,
            repaired_faces: 0,
            removed_vertices: 0,
        };
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                return Err(CommandError::Usage(USAGE));
            }
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::Usage(
                    "CullDegenerateMeshFaces requires meshes",
                ));
            };
            let cleanup = mesh.culled_degenerate_faces()?;
            result
                .sources
                .push((object.id(), object.geometry_snapshot().clone()));
            result.removed_faces += cleanup.removed_faces;
            result.repaired_faces += cleanup.repaired_faces;
            result.removed_vertices += cleanup.removed_vertices;
            if cleanup.changed() {
                if let Some(mesh) = cleanup.mesh {
                    result
                        .replacements
                        .push((object.id(), Geometry::Mesh(mesh)));
                } else {
                    result.deleted.push(object.id());
                }
            }
        }
        if result.sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        Ok(result)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, USAGE, |doc| self.apply(doc))
    }
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if self.sources.iter().any(|(id, source)| {
            !doc.is_object_selectable(*id)
                || !doc
                    .object(*id)
                    .is_some_and(|o| o.geometry_snapshot() == source)
        }) {
            return Err(CommandError::Usage(
                "Mesh cleanup source changed; select meshes again",
            ));
        }
        doc.replace_object_geometries(self.replacements.clone())?;
        doc.delete_objects(self.deleted.iter().copied())?;
        Ok(format!(
            "Removed {} degenerate face(s), repaired {} quad(s), removed {} unused vertex occurrence(s); {} mesh(es) changed, {} empty mesh(es) deleted",
            self.removed_faces,
            self.repaired_faces,
            self.removed_vertices,
            self.replacements.len(),
            self.deleted.len()
        ))
    }
}
impl Command for CullDegenerateMeshFacesCommand {
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
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        require_consumed(args, 0, USAGE)?;
        DegenerateFaceSelection::prepare(doc)?.apply(doc)
    }
}
#[cfg(test)]
mod tests;
