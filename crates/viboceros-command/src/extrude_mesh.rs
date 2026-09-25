use super::*;
use viboceros_geometry::{MeshExtrudeDirection, MeshExtrudeSelection};

const USAGE: &str =
    "ExtrudeMesh distance [Faces=0,1|Edges=0,1] [Basis=WCS|UVN] [Direction=X|Y|Z|N|V|x,y,z]";

pub(super) struct ExtrudeMeshCommand;

impl Command for ExtrudeMeshCommand {
    fn name(&self) -> &'static str {
        "ExtrudeMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (selection, distance, direction) = parse(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedExtrudeMeshGeometry);
                };
                Ok((
                    object.id(),
                    Geometry::Mesh(mesh.extrude_mesh(
                        &selection,
                        distance,
                        direction,
                        document.tolerance(),
                    )?),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let count = staged.len();
        document.replace_object_geometries(staged)?;
        Ok(format!("Extruded {count} mesh(es)"))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(
    arguments: &[&str],
) -> Result<(MeshExtrudeSelection, Real, MeshExtrudeDirection), CommandError> {
    let (first, rest) = arguments.split_first().ok_or(CommandError::Usage(USAGE))?;
    let distance = if let Some((name, value)) = first.split_once('=') {
        if !option_name_eq(name, "Distance") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_finite_real(value)?
    } else {
        parse_finite_real(first)?
    };
    if distance == 0.0 {
        return Err(CommandError::Usage(USAGE));
    }
    let mut selection = None;
    let mut basis = None;
    let mut direction = None;
    for arg in rest {
        let (name, value) = arg.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "Faces") || option_name_eq(name, "Edges") {
            if selection.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let indices = value
                .split(',')
                .map(|item| {
                    item.trim_start_matches('_')
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(USAGE))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if indices.is_empty() {
                return Err(CommandError::Usage(USAGE));
            }
            selection = Some(if option_name_eq(name, "Faces") {
                MeshExtrudeSelection::Faces(indices)
            } else {
                MeshExtrudeSelection::BoundaryEdges(indices)
            });
        } else if option_name_eq(name, "Basis") {
            if basis.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "Direction") {
            if direction.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    let selection = selection.unwrap_or(MeshExtrudeSelection::AllFaces);
    let basis = basis.unwrap_or("UVN");
    let direction = direction.unwrap_or(if option_name_eq(basis, "WCS") {
        "Z"
    } else {
        "N"
    });
    let mode = if option_name_eq(basis, "UVN") {
        if option_name_eq(direction, "N") {
            MeshExtrudeDirection::VertexNormals
        } else if option_name_eq(direction, "V")
            && matches!(selection, MeshExtrudeSelection::BoundaryEdges(_))
        {
            MeshExtrudeDirection::EdgeOutward
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    } else if option_name_eq(basis, "WCS") {
        let vector = if option_name_eq(direction, "X") {
            [1.0, 0.0, 0.0]
        } else if option_name_eq(direction, "Y") {
            [0.0, 1.0, 0.0]
        } else if option_name_eq(direction, "Z") {
            [0.0, 0.0, 1.0]
        } else {
            let values = direction
                .split(',')
                .map(|item| item.parse::<Real>().map_err(|_| CommandError::Usage(USAGE)))
                .collect::<Result<Vec<_>, _>>()?;
            let [x, y, z] = values.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            [*x, *y, *z]
        };
        MeshExtrudeDirection::World(
            Vector3::try_from(vector)
                .and_then(Vector3::normalized_nonzero)
                .map_err(|_| CommandError::Usage(USAGE))?,
        )
    } else {
        return Err(CommandError::Usage(USAGE));
    };
    Ok((selection, distance, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    #[test]
    fn command_extrudes_face_and_edge_and_preserves_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new_faces(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        document.select_object(id, SelectionMode::Replace).unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "ExtrudeMesh 2 Basis=WCS Direction=Z Faces=0")
                .unwrap(),
            "Extruded 1 mesh(es)"
        );
        let Geometry::Mesh(output) = document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(output.face_count(), 5);
        assert_eq!(output.topology().boundary_edge_count(), 4);
        assert_eq!(document.undo_label(), Some("ExtrudeMesh"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh.clone())
        );
        assert_eq!(
            registry
                .execute(&mut document, "ExtrudeMesh 1 Edges=0 Basis=UVN Direction=V")
                .unwrap(),
            "Extruded 1 mesh(es)"
        );
        let Geometry::Mesh(output) = document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(output.face_count(), 2);
        assert_eq!(output.topology().boundary_edge_count(), 6);
        let before = output.clone();
        let point_id = document
            .add_geometry(Geometry::Point(Point3::try_new(5., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_objects_direct([id, point_id], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtrudeMesh 1 Basis=WCS Direction=Z"),
            Err(CommandError::UnsupportedExtrudeMeshGeometry)
        ));
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(before)
        );
        assert!(registry.execute(&mut document, "ExtrudeMesh 0").is_err());
    }
}
