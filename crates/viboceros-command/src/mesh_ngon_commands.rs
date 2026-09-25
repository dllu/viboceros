//! Add or remove logical planar polygons on selected meshes.

use super::*;

const ADD_USAGE: &str = "AddNgonsToMesh [PlanarTolerance=nonnegative-distance]";
const DELETE_USAGE: &str = "DeleteMeshNgons";

pub(super) struct AddNgonsToMeshCommand;
pub(super) struct DeleteMeshNgonsCommand;

impl Command for AddNgonsToMeshCommand {
    fn name(&self) -> &'static str {
        "AddNgonsToMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let tolerance =
            parse_planar_tolerance(arguments)?.unwrap_or(document.tolerance().absolute());
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedMeshNgonGeometry);
                };
                let (result, count) = mesh.add_planar_ngons(tolerance)?;
                Ok((object.id(), result, count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let count = staged.iter().map(|(_, _, count)| count).sum::<usize>();
        let meshes = staged.iter().filter(|(_, _, count)| *count != 0).count();
        document.replace_object_geometries(
            staged
                .into_iter()
                .filter(|(_, _, count)| *count != 0)
                .map(|(id, mesh, _)| (id, Geometry::Mesh(mesh))),
        )?;
        Ok(format!("Added {count} mesh n-gon(s) in {meshes} mesh(es)"))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse_planar_tolerance(arguments)?;
        Ok(Some(mesh_selection_prompt(self.name())))
    }
}

impl Command for DeleteMeshNgonsCommand {
    fn name(&self) -> &'static str {
        "DeleteMeshNgons"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(DELETE_USAGE));
        }
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedMeshNgonGeometry);
                };
                Ok((object.id(), mesh.without_ngons(), mesh.ngons().len()))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let count = staged.iter().map(|(_, _, count)| count).sum::<usize>();
        let meshes = staged.iter().filter(|(_, _, count)| *count != 0).count();
        document.replace_object_geometries(
            staged
                .into_iter()
                .filter(|(_, _, count)| *count != 0)
                .map(|(id, mesh, _)| (id, Geometry::Mesh(mesh))),
        )?;
        Ok(format!(
            "Deleted {count} mesh n-gon(s) from {meshes} mesh(es)"
        ))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(DELETE_USAGE));
        }
        Ok(Some(mesh_selection_prompt(self.name())))
    }
}

fn parse_planar_tolerance(arguments: &[&str]) -> Result<Option<Real>, CommandError> {
    let [argument] = arguments else {
        return if arguments.is_empty() {
            Ok(None)
        } else {
            Err(CommandError::Usage(ADD_USAGE))
        };
    };
    let (key, value) = argument
        .split_once('=')
        .ok_or(CommandError::Usage(ADD_USAGE))?;
    if !option_name_eq(key, "PlanarTolerance") {
        return Err(CommandError::Usage(ADD_USAGE));
    }
    let value = value
        .parse::<Real>()
        .map_err(|_| CommandError::Usage(ADD_USAGE))?;
    if !value.is_finite() || value < 0.0 {
        return Err(CommandError::Usage(ADD_USAGE));
    }
    Ok(Some(value))
}

fn mesh_selection_prompt(command: &'static str) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command,
        filter: ObjectSelectionFilter::Mesh,
        workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        menus: vec![],
        choices: vec![],
        options: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn square() -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 2., 0.),
                point(0., 2., 0.),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn add_and_delete_preserve_mesh_identity_attributes_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let id = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let group = document.add_group(None, [id]).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "AddNgonsToMesh PlanarTolerance=0")
                .unwrap(),
            "Added 1 mesh n-gon(s) in 1 mesh(es)"
        );
        let object = document.object(id).unwrap();
        assert!(object.group_ids().contains(&group));
        assert!(document.is_selected(id));
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(mesh.ngons().len(), 1);
        assert_eq!(
            mesh.visible_wireframe_lines(document.tolerance())
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            registry.execute(&mut document, "DeleteMeshNgons").unwrap(),
            "Deleted 1 mesh n-gon(s) from 1 mesh(es)"
        );
        let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert!(mesh.ngons().is_empty());
        document.undo().unwrap();
        let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(mesh.ngons().len(), 1);
        document.undo().unwrap();
        let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert!(mesh.ngons().is_empty());
    }

    #[test]
    fn late_nonmesh_and_bad_tolerance_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh_id = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(point(4., 0., 0.)))
            .unwrap();
        document
            .select_objects_direct([mesh_id, point_id], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "AddNgonsToMesh PlanarTolerance=-1",
            "AddNgonsToMesh PlanarTolerance=NaN",
            "AddNgonsToMesh PlanarTolerance=0",
            "DeleteMeshNgons",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            let Geometry::Mesh(mesh) = document.object(mesh_id).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert!(mesh.ngons().is_empty());
            assert_eq!(document.undo_label(), before.as_deref());
        }
    }

    #[test]
    fn prompt_filters_meshes_and_noop_preserves_redo() {
        let registry = CommandRegistry::with_builtins();
        for command in ["AddNgonsToMesh", "DeleteMeshNgons"] {
            let prompt = registry.object_selection_prompt(command).unwrap().unwrap();
            assert_eq!(prompt.filter, ObjectSelectionFilter::Mesh);
        }
        let mut document = Document::default();
        let id = document.add_geometry(Geometry::Mesh(square())).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "AddNgonsToMesh").unwrap();
        document.undo().unwrap();
        assert_eq!(
            registry.execute(&mut document, "DeleteMeshNgons").unwrap(),
            "Deleted 0 mesh n-gon(s) from 0 mesh(es)"
        );
        assert!(document.can_redo());
    }
}
