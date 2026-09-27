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
            filter: ObjectSelectionFilter::PolygonCount,
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
        let tolerance = document.tolerance();
        for object in selected {
            let generated;
            let mesh = match object.geometry() {
                Geometry::Mesh(mesh) => mesh,
                Geometry::NurbsSurface(surface) if surface.plane(tolerance)?.is_some() => {
                    generated = surface.polygon_mesh(0.5, true, tolerance)?;
                    &generated
                }
                Geometry::Brep(brep)
                    if brep.faces().iter().try_fold(true, |planar, face| {
                        Ok::<bool, GeometryError>(
                            planar && face.surface().plane(tolerance)?.is_some(),
                        )
                    })? =>
                {
                    generated = brep.polygon_mesh(0.5, true, false, tolerance)?;
                    &generated
                }
                _ => {
                    return Err(CommandError::Usage(
                        "PolygonCount requires a mesh or planar surface/B-rep",
                    ));
                }
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
        assert_eq!(prompt.filter, ObjectSelectionFilter::PolygonCount);
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
    fn counts_planar_surface_and_box_faces_like_rhino() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let surface = NurbsSurface::try_bilinear([
            point(0., 0.),
            point(2., 0.),
            point(2., 3.),
            point(0., 3.),
        ])
        .unwrap();
        let plane = document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        let solid = Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 2.], [0., 3.], [0., 4.]],
            document.tolerance(),
        )
        .unwrap();
        let box_id = document.add_geometry(Geometry::Brep(solid)).unwrap();
        document
            .select_objects_direct([plane], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "PolygonCount").unwrap(),
            "There are 1 quadrilateral polygons and 0 triangular polygons in this selection\nThere would be 2 total triangular polygons in this selection after forced triangulation"
        );
        document
            .select_objects_direct([box_id], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "PolygonCount").unwrap(),
            "There are 6 quadrilateral polygons and 0 triangular polygons in this selection\nThere would be 12 total triangular polygons in this selection after forced triangulation"
        );
        document
            .select_objects_direct([plane, box_id], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "PolygonCount").unwrap(),
            "There are 7 quadrilateral polygons and 0 triangular polygons in this selection\nThere would be 14 total triangular polygons in this selection after forced triangulation"
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
        let sphere =
            NurbsSurface::try_sphere(CommandContext::default().construction_plane, 1.0).unwrap();
        let sphere_id = document
            .add_geometry(Geometry::NurbsSurface(sphere))
            .unwrap();
        document
            .select_objects_direct([sphere_id], SelectionMode::Replace)
            .unwrap();
        assert!(registry.execute(&mut document, "PolygonCount").is_err());
    }
}
