//! Extract stored mesh faces using their unsigned polygon areas.

use super::*;
use viboceros_geometry::MeshFace;

const USAGE: &str = "ExtractMeshFacesByArea (LargerThan=area|SmallerThan=area) [LargerThan=area] [SmallerThan=area] [MakeCopy=Yes|No] [BorderOnly=Yes|No]";

#[derive(Clone, Copy)]
struct Options {
    larger_than: Option<Real>,
    smaller_than: Option<Real>,
    make_copy: bool,
    border_only: bool,
}

pub(super) struct ExtractMeshFacesByAreaCommand;

impl Command for ExtractMeshFacesByAreaCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshFacesByArea"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        mesh_face_filter::extract_filtered_mesh_faces(
            document,
            "ExtractMeshFacesByArea",
            mesh_face_filter::FilterOutputOptions {
                make_copy: options.make_copy,
                border_only: options.border_only,
            },
            CommandError::UnsupportedExtractMeshFacesByAreaGeometry,
            CommandError::NoMeshFacesInAreaRange,
            CommandError::NoMeshFaceAreaBorders,
            |mesh, index| {
                let area = rhino_face_area(mesh, index)?;
                Ok(options.larger_than.is_none_or(|minimum| area >= minimum)
                    && options.smaller_than.is_none_or(|maximum| area <= maximum))
            },
        )
    }
}

// Rhino's area filter rounds a 2-by-2 square face just below 4, and a
// right triangle with legs of length 2 just below 2. Heron's formula gives
// those same values; the cross-product area remains available in the geometry
// kernel for numerically stable mass properties.
fn rhino_face_area(mesh: &TriangleMesh, index: usize) -> Result<Real, GeometryError> {
    let vertices = mesh.vertices();
    let triangle_area =
        |[a, b, c]: [u32; 3]| -> Result<Real, GeometryError> {
            let (a, b, c) = (
                vertices[a as usize],
                vertices[b as usize],
                vertices[c as usize],
            );
            let (ab, bc, ca) = (a.distance_to(b)?, b.distance_to(c)?, c.distance_to(a)?);
            let half_perimeter = (ab + bc + ca) / 2.0;
            Ok((half_perimeter
                * (half_perimeter - ab)
                * (half_perimeter - bc)
                * (half_perimeter - ca))
                .max(0.0)
                .sqrt())
        };
    let area = match mesh.faces()[index] {
        MeshFace::Triangle(face) => triangle_area(face)?,
        MeshFace::Quad([a, b, c, d]) => {
            if vertices[a as usize].distance_to(vertices[c as usize])?
                <= vertices[b as usize].distance_to(vertices[d as usize])?
            {
                triangle_area([a, b, c])? + triangle_area([a, c, d])?
            } else {
                triangle_area([a, b, d])? + triangle_area([b, c, d])?
            }
        }
    };
    if area.is_finite() {
        Ok(area)
    } else {
        mesh.face_area(index)
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut larger_than = None;
    let mut smaller_than = None;
    let mut make_copy = None;
    let mut border_only = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if (key.eq_ignore_ascii_case("LargerThan") || key.eq_ignore_ascii_case("MinArea"))
            && larger_than.is_none()
        {
            larger_than = Some(parse_area(value)?);
        } else if (key.eq_ignore_ascii_case("SmallerThan") || key.eq_ignore_ascii_case("MaxArea"))
            && smaller_than.is_none()
        {
            smaller_than = Some(parse_area(value)?);
        } else if key.eq_ignore_ascii_case("MakeCopy") && make_copy.is_none() {
            make_copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("BorderOnly") && border_only.is_none() {
            border_only = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    if larger_than.is_none() && smaller_than.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    if larger_than
        .zip(smaller_than)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Options {
        larger_than,
        smaller_than,
        make_copy: make_copy.unwrap_or(false),
        border_only: border_only.unwrap_or(false),
    })
}

fn parse_area(value: &str) -> Result<Real, CommandError> {
    value
        .parse::<Real>()
        .ok()
        .filter(|area| area.is_finite() && *area >= 0.0)
        .ok_or(CommandError::Usage(USAGE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn selected_mesh(document: &mut Document) -> ObjectId {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
                point(0.0, 2.0),
                point(3.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([1, 4, 2])],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        id
    }

    #[test]
    fn extracts_area_range_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        assert_eq!(
            registry
                .execute(&mut document, "ExtractMeshFacesByArea SmallerThan=2")
                .unwrap(),
            "Extracted 1 mesh face(s) from 1 mesh(es); source faces removed"
        );
        assert_eq!(document.objects().count(), 2);
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(remainder.face_count(), 1);
        assert!(remainder.faces()[0].is_quad());
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_area(0).unwrap(), 1.0);
        assert_eq!(document.undo_label(), Some("ExtractMeshFacesByArea"));
        document.undo().unwrap();
        assert_eq!(document.objects().count(), 1);
        let Geometry::Mesh(restored) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(restored.face_count(), 2);
    }

    #[test]
    fn copy_and_border_modes_preserve_source_and_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let group = document.add_group(None, [source]).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshFacesByArea LargerThan=0 SmallerThan=2 MakeCopy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].group_ids().contains(&group));
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 2);

        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractMeshFacesByArea SmallerThan=2 BorderOnly=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(matches!(selected[0].geometry(), Geometry::Polyline(_)));
        assert!(selected[0].group_ids().contains(&group));
        let Geometry::Mesh(original) = document.object(source).unwrap().geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(original.face_count(), 2);
    }

    #[test]
    fn invalid_range_or_late_nonmesh_has_no_edits() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let point_id = document
            .add_geometry(Geometry::Point(point(10.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([source, point_id], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for command in [
            "ExtractMeshFacesByArea SmallerThan=-1",
            "ExtractMeshFacesByArea LargerThan=3 SmallerThan=2",
            "ExtractMeshFacesByArea SmallerThan=2 SmallerThan=4",
            "ExtractMeshFacesByArea SmallerThan=2",
        ] {
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(document.objects().count(), 2);
            assert_eq!(document.undo_label(), before.as_deref());
        }
    }

    #[test]
    fn inclusive_thresholds_extract_equal_area_faces() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_mesh(&mut document);
        let before = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "ExtractMeshFacesByArea SmallerThan=0.9999"),
            Err(CommandError::NoMeshFacesInAreaRange)
        ));
        assert_eq!(document.objects().count(), 1);
        assert_eq!(document.undo_label(), before.as_deref());
        assert_eq!(document.selected_objects().next().unwrap().id(), source);
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractMeshFacesByArea SmallerThan=1 MakeCopy=Yes"
                )
                .unwrap(),
            "Extracted 1 mesh face(s) from 1 mesh(es); source faces copied"
        );
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        let Geometry::Mesh(extracted) = selected[0].geometry() else {
            panic!("mesh expected")
        };
        assert_eq!(extracted.face_area(0).unwrap(), 1.0);
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractMeshFacesByArea LargerThan=1 SmallerThan=1 MakeCopy=Yes"
                )
                .unwrap(),
            "Extracted 1 mesh face(s) from 1 mesh(es); source faces copied"
        );
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "ExtractMeshFacesByArea LargerThan=1")
                .unwrap(),
            "Extracted 2 mesh face(s) from 1 mesh(es); source faces removed"
        );
    }

    #[test]
    fn heron_rounding_matches_rhino_at_exact_thresholds() {
        let registry = CommandRegistry::with_builtins();
        for (vertices, face, threshold) in [
            (
                vec![point(0.0, 0.0), point(2.0, 0.0), point(0.0, 2.0)],
                MeshFace::Triangle([0, 1, 2]),
                2.0,
            ),
            (
                vec![
                    point(0.0, 0.0),
                    point(2.0, 0.0),
                    point(2.0, 2.0),
                    point(0.0, 2.0),
                ],
                MeshFace::Quad([0, 1, 2, 3]),
                4.0,
            ),
        ] {
            let mut document = Document::default();
            let mesh =
                TriangleMesh::try_new_faces(vertices, vec![face], document.tolerance()).unwrap();
            let exact_area = mesh.face_area(0).unwrap();
            assert_eq!(exact_area, threshold);
            assert!(rhino_face_area(&mesh, 0).unwrap() < threshold);
            let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
            document
                .select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
            assert!(matches!(
                registry.execute(
                    &mut document,
                    &format!("ExtractMeshFacesByArea LargerThan={threshold}")
                ),
                Err(CommandError::NoMeshFacesInAreaRange)
            ));
            assert!(
                registry
                    .execute(
                        &mut document,
                        &format!("ExtractMeshFacesByArea SmallerThan={threshold}")
                    )
                    .is_ok()
            );
        }
    }

    #[test]
    fn nonplanar_quad_uses_shorter_diagonal_for_area_filter() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(3.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 1.0, 2.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            document.tolerance(),
        )
        .unwrap();
        assert!(rhino_face_area(&mesh, 0).unwrap() > 6.1);
        assert!(rhino_face_area(&mesh, 0).unwrap() < 6.4);
        let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert!(
            registry
                .execute(
                    &mut document,
                    "ExtractMeshFacesByArea LargerThan=6.1 MakeCopy=Yes"
                )
                .is_ok()
        );
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtractMeshFacesByArea LargerThan=6.4"),
            Err(CommandError::NoMeshFacesInAreaRange)
        ));
    }
}
