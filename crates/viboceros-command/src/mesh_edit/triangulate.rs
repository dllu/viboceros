//! Whole-mesh triangulation with staged replacements.
use crate::*;

pub(crate) struct TriangulateMeshCommand;

impl Command for TriangulateMeshCommand {
    fn name(&self) -> &'static str {
        "TriangulateMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "TriangulateMesh")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedTriangulateMeshGeometry);
                };
                let (triangulated, quad_count) = mesh.triangulate_quads(document.tolerance())?;
                Ok((object.id(), triangulated, quad_count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let changed_mesh_count = staged
            .iter()
            .filter(|(_, _, quad_count)| *quad_count > 0)
            .count();
        let converted_quad_count = staged
            .iter()
            .map(|(_, _, quad_count)| quad_count)
            .sum::<usize>();
        document.replace_object_geometries(
            staged
                .iter()
                .filter(|(_, _, quad_count)| *quad_count > 0)
                .map(|(id, mesh, _)| (*id, Geometry::Mesh(mesh.clone()))),
        )?;
        Ok(format!(
            "Triangulated {converted_quad_count} quadrilateral face(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            staged.len() - changed_mesh_count
        ))
    }
}
