//! Select against the volume enclosed by a closed mesh or B-rep.

use super::selection_pipe::{interpolate, segment_distance};
use super::selection_volume::{VolumeRelation, parse_volume_mode};
use super::*;
use std::borrow::Cow;
use viboceros_geometry::{MeshSolid, SolidPointLocation, TriangleMesh};

const USAGE: &str =
    "SelVolumeObject [object-id] [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
const CURVE_SAMPLES: usize = 128;
const SURFACE_SAMPLES_PER_SPAN: usize = 16;

pub(super) struct SelVolumeObjectCommand;

impl Command for SelVolumeObjectCommand {
    fn name(&self) -> &'static str {
        "SelVolumeObject"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (source_id, options) = if let Some(id) = arguments
            .first()
            .and_then(|value| value.parse::<ObjectId>().ok())
        {
            (id, &arguments[1..])
        } else {
            let selected = document.selected_object_ids().collect::<Vec<_>>();
            let [id] = selected.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            (*id, arguments)
        };
        let mode = match options {
            [] => interface::RectSelectionMode::Crossing,
            [option] => parse_volume_mode(option, USAGE)?,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let source_mesh = match source {
            Geometry::Mesh(mesh) => Cow::Borrowed(mesh),
            Geometry::Brep(brep) if brep.is_closed() => {
                Cow::Owned(brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            Geometry::NurbsSurface(surface) => {
                Cow::Owned(surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let solid = MeshSolid::from_closed_mesh(&source_mesh).ok_or(CommandError::Usage(USAGE))?;
        let volume = SelectionObject {
            mesh: &source_mesh,
            solid,
            tolerance,
        };
        let mut ids = Vec::new();
        for object in document.selectable_objects() {
            if object.id() == source_id {
                continue;
            }
            if let Geometry::PointCloud(cloud) = object.geometry()
                && cloud.hidden_count() == cloud.points().len()
            {
                continue;
            }
            if volume.classify(object.geometry())?.selected(mode) {
                ids.push(object.id());
            }
        }
        let count = document.select_objects(ids, SelectionMode::Replace)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelectionObject<'a> {
    mesh: &'a TriangleMesh,
    solid: MeshSolid<'a>,
    tolerance: Tolerance,
}

impl SelectionObject<'_> {
    fn contains(&self, point: Point3) -> Result<bool, CommandError> {
        Ok(self.solid.classify_point(point, self.tolerance)? != SolidPointLocation::Outside)
    }

    fn segment_hits_source_surface(
        &self,
        start: Point3,
        end: Point3,
    ) -> Result<bool, CommandError> {
        let direction = start.vector_to(end)?;
        self.solid
            .any_segment_face_candidate(start, end, self.tolerance, |face| {
                segment_hits_mesh_face_surface(
                    self.mesh,
                    face,
                    start,
                    end,
                    direction,
                    self.tolerance,
                )
            })
    }

    fn disjoint_bounds(&self, geometry: &Geometry) -> bool {
        let source = self.mesh.bounds();
        let target = geometry.bounds();
        let a = source.min().to_array();
        let b = source.max().to_array();
        let c = target.min().to_array();
        let d = target.max().to_array();
        let epsilon = self.tolerance.absolute();
        (0..3).any(|axis| {
            (d[axis] < a[axis] && a[axis] - d[axis] > epsilon)
                || (c[axis] > b[axis] && c[axis] - b[axis] > epsilon)
        })
    }

    fn classify(&self, geometry: &Geometry) -> Result<VolumeRelation, CommandError> {
        if !matches!(geometry, Geometry::PointCloud(_)) && self.disjoint_bounds(geometry) {
            return Ok(VolumeRelation::OUTSIDE);
        }
        match geometry {
            Geometry::Point(point) => Ok(if self.contains(*point)? {
                VolumeRelation::INSIDE
            } else {
                VolumeRelation::OUTSIDE
            }),
            Geometry::PointCloud(cloud) => self.classify_points(
                cloud
                    .points()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, point)| (!cloud.is_hidden(index)).then_some(*point)),
            ),
            Geometry::Line(line) => self.classify_polyline(&[line.start(), line.end()]),
            Geometry::Polyline(polyline) => self.classify_polyline(polyline.vertices()),
            Geometry::Mesh(mesh) => self.classify_mesh(mesh),
            Geometry::NurbsSurface(surface) => {
                self.classify_mesh(&surface.tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)?)
            }
            Geometry::Brep(brep) => {
                self.classify_mesh(&brep.tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)?)
            }
            Geometry::Circle(_)
            | Geometry::Arc(_)
            | Geometry::Ellipse(_)
            | Geometry::NurbsCurve(_)
            | Geometry::PolyCurve(_) => {
                let curve = geometry.curve_ref().expect("curve geometry");
                self.classify_polyline(&curve.sample_equal_length_points(
                    CURVE_SAMPLES,
                    true,
                    self.tolerance,
                )?)
            }
        }
    }

    fn classify_points(
        &self,
        points: impl IntoIterator<Item = Point3>,
    ) -> Result<VolumeRelation, CommandError> {
        let mut any = false;
        let mut window = true;
        let mut crossing = false;
        for point in points {
            any = true;
            let inside = self.contains(point)?;
            window &= inside;
            crossing |= inside;
        }
        Ok(VolumeRelation {
            window: any && window,
            crossing,
        })
    }

    fn classify_polyline(&self, points: &[Point3]) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(points.iter().copied())?;
        let mut window = vertices.window;
        let mut crossing = vertices.crossing;
        for pair in points.windows(2) {
            if window {
                for step in 1..16 {
                    if !self.contains(interpolate(pair[0], pair[1], step as Real / 16.0)?)? {
                        window = false;
                        break;
                    }
                }
            }
            if !crossing && self.segment_hits_source_surface(pair[0], pair[1])? {
                crossing = true;
            }
            if crossing && !window {
                break;
            }
        }
        Ok(VolumeRelation { window, crossing })
    }

    fn classify_mesh(&self, target: &TriangleMesh) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(target.vertices().iter().copied())?;
        let mut window = vertices.window;
        let mut crossing = vertices.crossing;
        for face in target.faces() {
            let indices = face.indices();
            for edge in 0..indices.len() {
                let a = target.vertices()[indices[edge] as usize];
                let b = target.vertices()[indices[(edge + 1) % indices.len()] as usize];
                if window {
                    for step in 1..16 {
                        if !self.contains(interpolate(a, b, step as Real / 16.0)?)? {
                            window = false;
                            break;
                        }
                    }
                }
                if !crossing && self.segment_hits_source_surface(a, b)? {
                    crossing = true;
                }
            }
        }
        if window || !crossing {
            for triangle in 0..target.triangles().len() {
                let Some([a, b, c]) = target.triangle_points(triangle) else {
                    continue;
                };
                let center = Point3::try_new(
                    (a.x() + b.x() + c.x()) / 3.0,
                    (a.y() + b.y() + c.y()) / 3.0,
                    (a.z() + b.z() + c.z()) / 3.0,
                )?;
                let inside = self.contains(center)?;
                window &= inside;
                crossing |= inside;
            }
        }
        if !crossing {
            for face in self.mesh.faces() {
                let indices = face.indices();
                for edge in 0..indices.len() {
                    let a = self.mesh.vertices()[indices[edge] as usize];
                    let b = self.mesh.vertices()[indices[(edge + 1) % indices.len()] as usize];
                    if segment_hits_mesh_surface(target, a, b, self.tolerance)? {
                        crossing = true;
                        break;
                    }
                }
                if crossing {
                    break;
                }
            }
        }
        Ok(VolumeRelation { window, crossing })
    }
}

fn segment_hits_mesh_surface(
    mesh: &TriangleMesh,
    start: Point3,
    end: Point3,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let direction = start.vector_to(end)?;
    for face in 0..mesh.faces().len() {
        if segment_hits_mesh_face_surface(mesh, face, start, end, direction, tolerance)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn segment_hits_mesh_face_surface(
    mesh: &TriangleMesh,
    face: usize,
    start: Point3,
    end: Point3,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let indices = mesh.faces()[face].indices();
    let vertices = mesh.vertices();
    for triangle in [[0, 1, 2], [0, 2, 3]] {
        if triangle[2] >= indices.len() {
            continue;
        }
        let [a, b, c] = triangle.map(|corner| vertices[indices[corner] as usize]);
        let normal = a.vector_to(b)?.cross(a.vector_to(c)?)?;
        let denominator = normal.dot(direction)?;
        if denominator != 0.0 {
            let parameter = normal.dot(start.vector_to(a)?)? / denominator;
            if (0.0..=1.0).contains(&parameter) {
                let point = interpolate(start, end, parameter)?;
                if point.distance_to(mesh.closest_point_on_face(face, point)?)?
                    <= tolerance.absolute()
                {
                    return Ok(true);
                }
            }
        }
    }
    for edge in 0..indices.len() {
        let first = vertices[indices[edge] as usize];
        let second = vertices[indices[(edge + 1) % indices.len()] as usize];
        if segment_distance(start, end, first, second)? <= tolerance.absolute() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{Brep, Frame3, LineSegment, MeshFace, NurbsSurface, Vector3};

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame() -> Frame3 {
        Frame3::try_from_directions(
            p(0., 0., 0.),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn box_mesh() -> TriangleMesh {
        TriangleMesh::try_box_grid(
            frame(),
            [[0., 2.], [0., 2.], [0., 2.]],
            1,
            1,
            1,
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn line(a: Point3, b: Point3) -> Geometry {
        Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
    }

    #[test]
    fn segment_crosses_second_triangle_of_warped_quad() {
        let mesh = TriangleMesh::try_new_faces(
            vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 1.)],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            segment_hits_mesh_surface(
                &mesh,
                p(0.2, 0.8, 0.7),
                p(0.2, 0.8, 0.5),
                Tolerance::DEFAULT,
            )
            .unwrap()
        );
    }

    #[test]
    fn source_segment_candidates_match_full_face_scan() {
        let mesh = TriangleMesh::try_box_grid(
            frame(),
            [[0., 2.], [0., 2.], [0., 2.]],
            16,
            16,
            1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let volume = SelectionObject {
            mesh: &mesh,
            solid: MeshSolid::from_closed_mesh(&mesh).unwrap(),
            tolerance: Tolerance::DEFAULT,
        };
        for x in -1..=5 {
            for y in -1..=5 {
                let x = x as Real * 0.4;
                let y = y as Real * 0.4;
                for (start, end) in [
                    (p(x, y, -1.), p(x, y, 3.)),
                    (p(-1., y, x), p(3., y, x)),
                    (p(x, y, 0.25), p(x + 0.2, y + 0.1, 0.75)),
                ] {
                    assert_eq!(
                        volume.segment_hits_source_surface(start, end).unwrap(),
                        segment_hits_mesh_surface(&mesh, start, end, Tolerance::DEFAULT).unwrap(),
                        "{start:?} to {end:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn mesh_volume_selects_points_lines_and_all_four_modes() {
        let mut document = Document::default();
        let source = document.add_geometry(Geometry::Mesh(box_mesh())).unwrap();
        let inside = document
            .add_geometry(Geometry::Point(p(1., 1., 1.)))
            .unwrap();
        let boundary = document
            .add_geometry(Geometry::Point(p(0., 1., 1.)))
            .unwrap();
        let near_boundary = document
            .add_geometry(Geometry::Point(p(
                -document.tolerance().absolute() * 0.5,
                1.,
                1.,
            )))
            .unwrap();
        let outside = document
            .add_geometry(Geometry::Point(p(3., 3., 3.)))
            .unwrap();
        let crossing = document
            .add_geometry(line(p(-1., 1., 1.), p(3., 1., 1.)))
            .unwrap();
        let enclosed = document
            .add_geometry(line(p(0.5, 0.5, 0.5), p(1.5, 1.5, 1.5)))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        for (mode, expected) in [
            ("Window", vec![inside, boundary, near_boundary, enclosed]),
            (
                "Crossing",
                vec![inside, boundary, near_boundary, crossing, enclosed],
            ),
            ("InvertWindow", vec![outside]),
            ("InvertCrossing", vec![outside, crossing]),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("SelVolumeObject {source} SelectionMode={mode}"),
                )
                .unwrap();
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                expected.into_iter().collect(),
                "{mode}"
            );
        }
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "SelVolumeObject").unwrap();
        assert!(document.is_selected(inside));
        assert!(!document.is_selected(source));
    }

    #[test]
    fn brep_box_source_and_face_piercing_mesh_target() {
        let mut document = Document::default();
        let brep =
            Brep::try_box(frame(), [[0., 2.], [0., 2.], [0., 2.]], Tolerance::DEFAULT).unwrap();
        let source = document.add_geometry(Geometry::Brep(brep)).unwrap();
        let target = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new_faces(
                    vec![p(-1., -1., 1.), p(3., -1., 1.), p(1., 3., 1.)],
                    vec![MeshFace::Triangle([0, 1, 2])],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("SelVolumeObject {source}"))
            .unwrap();
        assert!(document.is_selected(target));
    }

    #[test]
    fn closed_nurbs_sphere_source_selects_center_point() {
        let mut document = Document::default();
        let sphere = NurbsSurface::try_sphere(frame(), 2.).unwrap();
        let tessellation = sphere.tessellate(16, Tolerance::DEFAULT).unwrap();
        assert!(tessellation.topology().is_solid());
        let source = document
            .add_geometry(Geometry::NurbsSurface(sphere))
            .unwrap();
        let center = document
            .add_geometry(Geometry::Point(p(0., 0., 0.)))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("SelVolumeObject {source}"))
            .unwrap();
        assert!(document.is_selected(center));
    }

    #[test]
    fn torus_window_rejects_line_through_hole_despite_inside_endpoints() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame(), 3., 1.).unwrap(),
            ))
            .unwrap();
        let through_hole = document
            .add_geometry(line(p(-3., 0., 0.), p(3., 0., 0.)))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut document,
                &format!("SelVolumeObject {source} SelectionMode=Window"),
            )
            .unwrap();
        assert!(!document.is_selected(through_hole));
        registry
            .execute(&mut document, &format!("SelVolumeObject {source}"))
            .unwrap();
        assert!(document.is_selected(through_hole));
    }

    #[test]
    fn open_source_mesh_is_rejected_without_editing_selection() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        assert!(registry.execute(&mut document, "SelVolumeObject").is_err());
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![source]
        );
    }
}
