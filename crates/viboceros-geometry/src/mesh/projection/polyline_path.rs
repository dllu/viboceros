//! Directed plane sections for polylines drawn on polygon meshes.
use super::*;
use crate::{Frame3, Polyline3};
mod boundary;
pub(super) use boundary::Boundaries;

impl MeshProjectionIndex<'_> {
    /// Connects projected locations with a section plane containing the chord
    /// and the first polygon face's normal. Source order therefore matters at folds.
    /// Shared exact mesh sections resolve edge/vertex crossings. When sections
    /// separate, a bounded graph follows naked boundaries and may connect
    /// original boundary corners across disconnected mesh components, matching
    /// the native drawing command. These bridges need not lie on the mesh.
    pub fn polyline_on_mesh_segment(
        &self,
        start: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Polyline3, GeometryError> {
        let failed = || GeometryError::MeshPolylinePathUnresolved;
        let a = self.closest_location(start)?.ok_or_else(failed)?;
        let b = self.closest_location(end)?.ok_or_else(failed)?;
        let first_face = self.triangle_faces[a.triangle_index];
        if first_face == self.triangle_faces[b.triangle_index] {
            // The native command connects locations within one original face
            // directly, including a nonplanar quad. Its internal display or
            // closest-point diagonal does not add a polyline corner.
            return Polyline3::try_new(vec![a.point, b.point], Tolerance::NUMERICAL_VALIDATION);
        }
        let normal = self.mesh.polygon_face_normal(first_face)?.as_vector();
        let chord = a.point.vector_to(b.point)?;
        let section_normal = chord.cross(normal)?;
        let frame = Frame3::try_from_x_and_normal(
            a.point,
            chord,
            section_normal,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let line = Polyline3::try_new(vec![a.point, b.point], Tolerance::NUMERICAL_VALIDATION)?
            .to_nurbs()?;
        let candidates = self.curve_candidates(&line, normal)?;
        let mut map = BTreeMap::<[u64; 3], u32>::new();
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for index in candidates {
            let mut triangle = [0; 3];
            for (j, raw) in self.triangles[index].iter().copied().enumerate() {
                let point = self.mesh.vertices()[raw as usize];
                let key = point
                    .to_array()
                    .map(super::super::canonical_coordinate_bits);
                triangle[j] = *map.entry(key).or_insert_with(|| {
                    let id = vertices.len() as u32;
                    vertices.push(point);
                    id
                });
            }
            triangles.push(triangle);
        }
        if triangles.is_empty() {
            return Err(failed());
        }
        let section_mesh =
            TriangleMesh::try_new(vertices, triangles, Tolerance::NUMERICAL_VALIDATION)?;
        let sections = section_mesh.section_with_plane(frame, Tolerance::NUMERICAL_VALIDATION)?;
        let mut best: Option<(Real, Polyline3)> = None;
        for section in &sections {
            let Some(u) = station(section, a.point, tolerance)? else {
                continue;
            };
            let Some(v) = station(section, b.point, tolerance)? else {
                continue;
            };
            let mut choices = vec![portion(section, u, v, a.point, b.point)?];
            if closed_section(section) {
                let count = section.segment_count() as Real;
                let (low, high) = if u < v { (u, v) } else { (v, u) };
                let mut points = vec![if u < v { b.point } else { a.point }];
                points.extend(
                    section
                        .vertices()
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| (*i as Real) > high && (*i as Real) < count)
                        .map(|(_, p)| *p),
                );
                points.push(section.vertices()[0]);
                points.extend(
                    section
                        .vertices()
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| (*i as Real) > 0. && (*i as Real) < low)
                        .map(|(_, p)| *p),
                );
                points.push(if u < v { a.point } else { b.point });
                if u < v {
                    points.reverse();
                }
                choices.push(compact(points)?);
            }
            for candidate in choices {
                let length = candidate.length()?;
                if best.as_ref().is_none_or(|(current, _)| length < *current) {
                    best = Some((length, candidate));
                }
            }
        }
        if let Some((_, path)) = best {
            return Ok(path);
        }
        boundary::route(self, &sections, a.point, b.point, tolerance)
    }
}
fn closed_section(curve: &Polyline3) -> bool {
    curve.vertices().first() == curve.vertices().last()
}
fn station(
    curve: &Polyline3,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Option<Real>, GeometryError> {
    let mut best = None;
    for (i, segment) in curve.segments().enumerate() {
        let q = segment.closest_point(point, Tolerance::NUMERICAL_VALIDATION)?;
        let error = q.distance_to(point)?;
        let fraction = segment.start().distance_to(q)? / segment.length()?;
        if best.is_none_or(|(_, d)| error < d) {
            best = Some((i as Real + fraction, error));
        }
    }
    Ok(best
        .filter(|(_, d)| *d <= tolerance.absolute())
        .map(|(t, _)| t))
}
fn portion(
    curve: &Polyline3,
    u: Real,
    v: Real,
    start: Point3,
    end: Point3,
) -> Result<Polyline3, GeometryError> {
    let mut middle = curve
        .vertices()
        .iter()
        .enumerate()
        .filter(|(i, _)| (*i as Real) > u.min(v) && (*i as Real) < u.max(v))
        .map(|(_, p)| *p)
        .collect::<Vec<_>>();
    if u > v {
        middle.reverse();
    }
    let mut points = vec![start];
    points.extend(middle);
    points.push(end);
    compact(points)
}
fn compact(points: Vec<Point3>) -> Result<Polyline3, GeometryError> {
    let mut output: Vec<Point3> = Vec::new();
    for point in points {
        if output
            .last()
            .is_none_or(|last| !last.is_near(point, Tolerance::NUMERICAL_VALIDATION))
        {
            output.push(point);
        }
    }
    if output.len() > crate::MAX_CURVE_DIVISION_POINTS {
        return Err(GeometryError::MeshPullLimit);
    }
    Polyline3::try_new(output, Tolerance::NUMERICAL_VALIDATION)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn fold() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                p(-4., -4., 0.),
                p(0., -4., 0.),
                p(4., -4., 3.),
                p(-4., 4., 0.),
                p(0., 4., 0.),
                p(4., 4., 3.),
            ],
            vec![MeshFace::Quad([0, 1, 4, 3]), MeshFace::Quad([1, 2, 5, 4])],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn ring() -> TriangleMesh {
        TriangleMesh::try_new_faces(
            vec![
                p(-4., -4., 0.),
                p(4., -4., 0.),
                p(4., 4., 0.),
                p(-4., 4., 0.),
                p(-1., -1., 0.),
                p(1., -1., 0.),
                p(1., 1., 0.),
                p(-1., 1., 0.),
            ],
            vec![
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn excessive_disconnected_components_fail_before_boundary_graph_expansion() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for i in 0..1025 {
            let x = 3. * i as Real;
            let first = vertices.len() as u32;
            vertices.extend([
                p(x, 0., 0.),
                p(x + 1., 0., 0.),
                p(x + 1., 1., 0.),
                p(x, 1., 0.),
            ]);
            faces.push(MeshFace::Quad([first, first + 1, first + 2, first + 3]));
        }
        let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
        let before = mesh.clone();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        assert!(
            index
                .polyline_on_mesh_segment(p(0.5, 0.5, 0.), p(3072.5, 0.5, 0.), Tolerance::DEFAULT)
                .is_err()
        );
        assert!(index.boundary_routes.get().unwrap().is_err());
        assert_eq!(mesh, before);
    }
    #[test]
    fn topologically_closed_boundaries_remain_routable_at_small_model_scales() {
        let base = ring();
        for scale in [1e-9, 1e-6, 1e6] {
            let mesh = TriangleMesh::try_new_faces(
                base.vertices()
                    .iter()
                    .map(|p| Point3::try_from(p.to_array().map(|x| x * scale)).unwrap())
                    .collect(),
                base.faces().to_vec(),
                Tolerance::MESH_VALIDATION,
            )
            .unwrap();
            let index = MeshProjectionIndex::new(&mesh).unwrap();
            let tolerance = Tolerance::try_new(1e-6 * scale, 1e-12, 1e-9).unwrap();
            let path = index
                .polyline_on_mesh_segment(
                    p(-3. * scale, 0.8 * scale, 0.),
                    p(3. * scale, 0.8 * scale, 0.),
                    tolerance,
                )
                .unwrap();
            assert!((path.length().unwrap() / scale - 6.4).abs() < 1e-9);
        }
    }
    #[test]
    fn hole_routes_follow_the_shorter_boundary_and_reuse_immutable_topology() {
        let mesh = ring();
        let before = mesh.clone();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        let mut cache = None;
        for y in [0.8, -0.8] {
            let path = index
                .polyline_on_mesh_segment(p(-3., y, 0.), p(3., y, 0.), Tolerance::DEFAULT)
                .unwrap();
            assert!((path.length().unwrap() - 6.4).abs() < 1e-12);
            let corner = p(-1., y.signum(), 0.);
            assert!(
                path.closest_point(corner, Tolerance::NUMERICAL_VALIDATION)
                    .unwrap()
                    .distance_to(corner)
                    .unwrap()
                    < 1e-12
            );
            for segment in path.segments() {
                for i in 0..=16 {
                    let point = segment.point_at(i as Real / 16.).unwrap();
                    assert!(
                        point
                            .distance_to(index.closest_point(point).unwrap().unwrap())
                            .unwrap()
                            < 1e-12
                    );
                }
            }
            let pointer = index.boundary_routes.get().unwrap() as *const _;
            if let Some(previous) = cache {
                assert_eq!(previous, pointer);
            }
            cache = Some(pointer);
        }
        assert_eq!(mesh, before);
    }
    #[test]
    fn disconnected_component_bridges_use_original_boundary_corners() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                p(-4., -4., 0.),
                p(-1., -4., 0.),
                p(-1., 4., 0.),
                p(-4., 4., 0.),
                p(1., -4., 0.),
                p(4., -4., 0.),
                p(4., 4., 0.),
                p(1., 4., 0.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        let path = index
            .polyline_on_mesh_segment(p(-3., 0., 0.), p(3., 1., 0.), Tolerance::DEFAULT)
            .unwrap();
        for corner in [p(-1., 4., 0.), p(1., 4., 0.)] {
            assert!(path.vertices().contains(&corner));
        }
        let midpoint = p(0., 4., 0.);
        assert!(
            path.closest_point(midpoint, Tolerance::NUMERICAL_VALIDATION)
                .unwrap()
                .distance_to(midpoint)
                .unwrap()
                < 1e-12
        );
        assert_eq!(
            index
                .closest_point(midpoint)
                .unwrap()
                .unwrap()
                .distance_to(midpoint)
                .unwrap(),
            1.
        );
    }
    #[test]
    fn coincident_unwelded_faces_share_a_section_without_changing_the_source() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                p(-4., -4., 0.),
                p(0., -4., 0.),
                p(0., 4., 0.),
                p(-4., 4., 0.),
                p(0., -4., 0.),
                p(4., -4., 3.),
                p(4., 4., 3.),
                p(0., 4., 0.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let before = mesh.clone();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        let path = index
            .polyline_on_mesh_segment(p(-3., -2., 0.), p(3., 2., 2.25), Tolerance::DEFAULT)
            .unwrap();
        let middle = p(0., 0., 0.);
        assert!(
            path.closest_point(middle, Tolerance::NUMERICAL_VALIDATION)
                .unwrap()
                .distance_to(middle)
                .unwrap()
                < 1e-12
        );
        assert_eq!(mesh, before);
    }
    #[test]
    fn one_warped_quad_uses_a_direct_chord_without_an_internal_diagonal_corner() {
        let mesh = TriangleMesh::try_new_faces(
            vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 4., 2.), p(0., 4., 0.)],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        let path = index
            .polyline_on_mesh_segment(p(0.5, 0.5, 0.), p(3.5, 3.5, 1.5), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(path.vertices().len(), 2);
        let middle = path.segments().next().unwrap().point_at(0.5).unwrap();
        assert!(
            middle
                .distance_to(index.closest_point(middle).unwrap().unwrap())
                .unwrap()
                > 0.1
        );
    }
    #[test]
    fn section_vertices_that_round_to_the_same_remote_corner_do_not_block_a_path() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let mesh = TriangleMesh::try_new_faces(
            vec![p(-4., -4.), p(4., -4.), p(4., 4.), p(-4., 4.)],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        for pair in [p(-3., -2.), p(2., -1.), p(1., 3.), p(-2., 2.)].windows(2) {
            let path = index
                .polyline_on_mesh_segment(pair[0], pair[1], Tolerance::DEFAULT)
                .unwrap();
            assert!((path.length().unwrap() - pair[0].distance_to(pair[1]).unwrap()).abs() < 1e-10);
        }
    }
    #[test]
    fn directed_fold_sections_are_asymmetric_and_stay_on_the_mesh() {
        let mesh = fold();
        let before = mesh.clone();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap();
        for (start, end, middle) in [
            (p(-3., -2., 0.), p(3., 2., 2.25), p(0., 0., 0.)),
            (p(3., 2., 2.25), p(-3., -2., 0.), p(0., -18. / 41., 0.)),
        ] {
            let path = index
                .polyline_on_mesh_segment(start, end, tolerance)
                .unwrap();
            assert!(
                path.closest_point(middle, Tolerance::NUMERICAL_VALIDATION)
                    .unwrap()
                    .distance_to(middle)
                    .unwrap()
                    < 1e-10
            );
            assert_eq!(path.vertices().first(), Some(&start));
            assert_eq!(path.vertices().last(), Some(&end));
            for segment in path.segments() {
                for i in 0..=16 {
                    let q = segment.point_at(i as Real / 16.).unwrap();
                    assert!(
                        index
                            .closest_point(q)
                            .unwrap()
                            .unwrap()
                            .distance_to(q)
                            .unwrap()
                            < 1e-10
                    );
                }
            }
        }
        assert_eq!(mesh, before);
    }
    #[test]
    fn directed_sections_preserve_small_large_and_translated_model_scales() {
        let base = fold();
        for (scale, offset) in [(1e-6, 0.), (1e6, 0.), (1., 1e9)] {
            let map =
                |p: Point3| Point3::try_from(p.to_array().map(|v| v * scale + offset)).unwrap();
            let mesh = TriangleMesh::try_new_faces(
                base.vertices().iter().copied().map(map).collect(),
                base.faces().to_vec(),
                Tolerance::try_new(1e-9 * scale, 1e-12, 1e-9).unwrap(),
            )
            .unwrap();
            let index = MeshProjectionIndex::new(&mesh).unwrap();
            let tolerance = Tolerance::try_new(1e-6 * scale, 1e-12, 1e-9).unwrap();
            let path = index
                .polyline_on_mesh_segment(map(p(-3., -2., 0.)), map(p(3., 2., 2.25)), tolerance)
                .unwrap_or_else(|e| panic!("scale={scale} offset={offset}: {e:?}"));
            let middle = map(p(0., 0., 0.));
            assert!(
                path.closest_point(middle, Tolerance::NUMERICAL_VALIDATION)
                    .unwrap()
                    .distance_to(middle)
                    .unwrap()
                    <= tolerance.absolute()
            );
        }
    }
}
