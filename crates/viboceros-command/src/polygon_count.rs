//! Rhino-compatible read-only triangle and quadrilateral mesh counts.

use super::*;

const USAGE: &str = "PolygonCount";

pub(super) struct PolygonCountCommand;

impl Command for PolygonCountCommand {
    fn name(&self) -> &'static str {
        "PolygonCount"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut selected = document.selected_objects().peekable();
        if selected.peek().is_none() {
            return Err(CommandError::NoObjectsSelected);
        }
        let (mut triangles, mut quads) = (0_usize, 0_usize);
        for object in selected {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::Usage("PolygonCount requires mesh objects"));
            };
            for face in mesh.faces() {
                if face.is_triangle() {
                    triangles = triangles.checked_add(1).ok_or(CommandError::Usage(USAGE))?;
                } else {
                    quads = quads.checked_add(1).ok_or(CommandError::Usage(USAGE))?;
                }
            }
        }
        let forced_triangles = triangles
            .checked_add(quads.checked_mul(2).ok_or(CommandError::Usage(USAGE))?)
            .ok_or(CommandError::Usage(USAGE))?;
        Ok(format!(
            "There are {quads} quadrilateral polygons and {triangles} triangular polygons in this selection\nThere would be {forced_triangles} total triangular polygons in this selection after forced triangulation"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn mixed_mesh() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                point(0., 0.),
                point(1., 0.),
                point(1., 1.),
                point(0., 1.),
                point(2., 0.),
                point(3., 0.),
                point(3., 1.),
                point(2., 1.),
            ],
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([0, 2, 3]),
                MeshFace::Quad([4, 5, 6, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn reports_rhino_polygon_counts_without_changing_mesh_or_selection() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let id = document.add_geometry(Geometry::Mesh(mixed_mesh())).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let original = document.object(id).unwrap().geometry().clone();
        let prompt = PolygonCountCommand
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap();
        assert_eq!(prompt.filter, ObjectSelectionFilter::Mesh);
        assert_eq!(
            prompt.workflow,
            ObjectSelectionWorkflow::ConfirmAfterSelection
        );
        assert_eq!(
            registry.execute(&mut document, "PolygonCount").unwrap(),
            "There are 1 quadrilateral polygons and 2 triangular polygons in this selection\nThere would be 4 total triangular polygons in this selection after forced triangulation"
        );
        assert_eq!(document.object(id).unwrap().geometry(), &original);
        assert!(document.is_selected(id));

        let Geometry::Mesh(mesh) = &original else {
            panic!("mesh expected")
        };
        let (with_ngon, count) = mesh.add_planar_ngons(0.0).unwrap();
        assert_eq!(count, 1);
        document
            .replace_object_geometries([(id, Geometry::Mesh(with_ngon))])
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "PolygonCount").unwrap(),
            "There are 1 quadrilateral polygons and 2 triangular polygons in this selection\nThere would be 4 total triangular polygons in this selection after forced triangulation"
        );
    }

    #[test]
    fn rejects_invalid_selection_and_arguments() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert!(matches!(
            registry.execute(&mut document, "PolygonCount"),
            Err(CommandError::NoObjectsSelected)
        ));
        assert!(
            registry
                .execute(&mut document, "PolygonCount Extra")
                .is_err()
        );
        let point_id = document
            .add_geometry(Geometry::Point(point(0., 0.)))
            .unwrap();
        document
            .select_objects_direct([point_id], SelectionMode::Replace)
            .unwrap();
        assert!(registry.execute(&mut document, "PolygonCount").is_err());
    }
}
