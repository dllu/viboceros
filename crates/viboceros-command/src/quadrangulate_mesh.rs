//! Scriptable triangle-to-quad mesh cleanup.

use super::*;

const USAGE: &str = "QuadrangulateMesh [Planarity=degrees] [Rectangularity=ratio]";

pub(super) struct QuadrangulateMeshCommand;

impl Command for QuadrangulateMeshCommand {
    fn name(&self) -> &'static str {
        "QuadrangulateMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut planarity = None;
        let mut rectangularity = None;
        for &argument in arguments {
            let Some((key, value)) = argument.split_once('=') else {
                return Err(CommandError::Usage(USAGE));
            };
            if key.eq_ignore_ascii_case("Planarity") && planarity.is_none() {
                planarity = Some(
                    value
                        .parse::<Real>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                );
            } else if key.eq_ignore_ascii_case("Rectangularity") && rectangularity.is_none() {
                rectangularity = Some(
                    value
                        .parse::<Real>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                );
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let planarity = planarity.unwrap_or(1.0);
        let rectangularity = rectangularity.unwrap_or(2.0);
        if !planarity.is_finite()
            || !(0.0..=180.0).contains(&planarity)
            || !rectangularity.is_finite()
            || rectangularity < 1.0
        {
            return Err(CommandError::Usage(USAGE));
        }
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedQuadrangulateMeshGeometry);
                };
                let (converted, count) =
                    mesh.quadrangulate_triangles(planarity, rectangularity, document.tolerance())?;
                Ok((object.id(), converted, count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let converted_faces = staged.iter().map(|(_, _, count)| count).sum::<usize>();
        let changed_meshes = staged.iter().filter(|(_, _, count)| *count > 0).count();
        document.replace_object_geometries(
            staged
                .into_iter()
                .filter(|(_, _, count)| *count > 0)
                .map(|(id, mesh, _)| (id, Geometry::Mesh(mesh))),
        )?;
        Ok(format!(
            "Quadrangulated {converted_faces} triangle pair(s) in {changed_meshes} mesh(es)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn square() -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn edits_selected_meshes_atomically_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let second = document.add_geometry(Geometry::Mesh(square())).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "QuadrangulateMesh Planarity=0 Rectangularity=1"
                )
                .unwrap(),
            "Quadrangulated 2 triangle pair(s) in 2 mesh(es)"
        );
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(mesh.face_count(), 1);
            assert!(mesh.faces()[0].is_quad());
        }
        assert_eq!(document.undo_label(), Some("QuadrangulateMesh"));
        document.undo().unwrap();
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(mesh.face_count(), 2);
        }
    }

    #[test]
    fn invalid_options_and_late_nonmesh_preserve_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let point = document
            .add_geometry(Geometry::Point(point(2.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([mesh, point], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "QuadrangulateMesh Planarity=-1",
            "QuadrangulateMesh Rectangularity=0.5",
            "QuadrangulateMesh Planarity=1 Planarity=2",
            "QuadrangulateMesh Planarity=1",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            let Geometry::Mesh(current) = document.object(mesh).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(current.face_count(), 2);
            assert_eq!(document.undo_label(), before.as_deref());
        }
    }
}
