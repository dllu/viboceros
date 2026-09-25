use super::*;
use viboceros_geometry::{NonPlanarQuadCriterion, QuadSplitMethod};

const USAGE: &str = "TriangulateNonPlanarQuads [Mode=Distance|Angle|Both] [Distance=value] [Angle=degrees] [SplitMethod=ShortestDiagonal|LongestDiagonal|MinimizeArea|MaximizeArea|MinimumAngle|MaximumAngle]";

pub(super) struct TriangulateNonPlanarQuadsCommand;

impl Command for TriangulateNonPlanarQuadsCommand {
    fn name(&self) -> &'static str {
        "TriangulateNonPlanarQuads"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (criterion, method) = parse_options(arguments, document.tolerance().absolute())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedTriangulateNonPlanarQuadsGeometry);
                };
                let (mesh, count) =
                    mesh.triangulate_nonplanar_quads(criterion, method, document.tolerance())?;
                Ok((object.id(), mesh, count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let converted = staged.iter().map(|(_, _, count)| count).sum::<usize>();
        let changed = staged.iter().filter(|(_, _, count)| *count > 0).count();
        document.replace_object_geometries(
            staged
                .iter()
                .filter(|(_, _, count)| *count > 0)
                .map(|(id, mesh, _)| (*id, Geometry::Mesh(mesh.clone()))),
        )?;
        Ok(format!(
            "Triangulated {converted} nonplanar quad(s) in {changed} mesh(es); {} mesh(es) unchanged",
            staged.len() - changed
        ))
    }
}

fn parse_options(
    arguments: &[&str],
    default_distance: Real,
) -> Result<(NonPlanarQuadCriterion, QuadSplitMethod), CommandError> {
    let mut mode = "Distance";
    let mut distance = default_distance;
    let mut angle = 1.0;
    let mut method = QuadSplitMethod::ShortestDiagonal;
    let mut seen = [false; 4];
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        let slot = if option_name_eq(name, "Mode") {
            0
        } else if option_name_eq(name, "Distance") {
            1
        } else if option_name_eq(name, "Angle") {
            2
        } else if option_name_eq(name, "SplitMethod") || option_name_eq(name, "Split") {
            3
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if seen[slot] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[slot] = true;
        match slot {
            0 => {
                mode = if option_name_eq(value, "Distance") {
                    "Distance"
                } else if option_name_eq(value, "Angle") {
                    "Angle"
                } else if option_name_eq(value, "Both") {
                    "Both"
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
            }
            1 => distance = parse_finite_real(value)?,
            2 => angle = parse_finite_real(value)?,
            _ => {
                method = if option_name_eq(value, "ShortestDiagonal") {
                    QuadSplitMethod::ShortestDiagonal
                } else if option_name_eq(value, "LongestDiagonal") {
                    QuadSplitMethod::LongestDiagonal
                } else if option_name_eq(value, "MinimizeArea") {
                    QuadSplitMethod::MinimizeArea
                } else if option_name_eq(value, "MaximizeArea") {
                    QuadSplitMethod::MaximizeArea
                } else if option_name_eq(value, "MinimumAngle") {
                    QuadSplitMethod::MinimumAngle
                } else if option_name_eq(value, "MaximumAngle") {
                    QuadSplitMethod::MaximumAngle
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
            }
        }
        index += consumed;
    }
    if distance <= 0. || !(0. ..=180.).contains(&angle) {
        return Err(CommandError::Usage(USAGE));
    }
    let angle = angle.to_radians();
    let criterion = match mode {
        "Distance" => NonPlanarQuadCriterion::Distance(distance),
        "Angle" => NonPlanarQuadCriterion::Angle(angle),
        _ => NonPlanarQuadCriterion::Both { distance, angle },
    };
    Ok((criterion, method))
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    #[test]
    fn command_is_atomic_preserves_identity_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let vertices = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.4]]
            .map(|p| Point3::try_from(p).unwrap());
        let mesh = TriangleMesh::try_new_faces(
            vertices.to_vec(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        let point = document
            .add_geometry(Geometry::Point(Point3::try_new(4., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_objects_direct([id, point], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "TriangulateNonPlanarQuads Distance=0.1"),
            Err(CommandError::UnsupportedTriangulateNonPlanarQuadsGeometry)
        ));
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh.clone())
        );
        document.select_object(id, SelectionMode::Replace).unwrap();
        assert_eq!(registry.execute(&mut document, "TriangulateNonPlanarQuads Mode=Both Distance=0.2 Angle=1 SplitMethod=LongestDiagonal").unwrap(), "Triangulated 1 nonplanar quad(s) in 1 mesh(es); 0 mesh(es) unchanged");
        let Geometry::Mesh(result) = document.object(id).unwrap().geometry() else {
            panic!("expected mesh");
        };
        assert_eq!(result.face_count(), 2);
        assert_eq!(document.undo_label(), Some("TriangulateNonPlanarQuads"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh)
        );
        assert!(
            registry
                .execute(&mut document, "TriangulateNonPlanarQuads Distance=0")
                .is_err()
        );
    }
}
