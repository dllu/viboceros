//! Command-compatible box topology over validated orthonormal frame intervals.
use super::*;
use crate::opennurbs_box_topology::{CORNERS, EDGES, FACES, parameter_extent};
impl Brep {
    /// Constructs a box with the OpenNURBS vertex/edge/face ordering, physical
    /// surface domains, and trim parameter intervals used by Rhino's Box.
    pub fn try_command_box(
        frame: Frame3,
        intervals: [[Real; 2]; 3],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(intervals.into_iter().flatten(), "command box intervals")?;
        if intervals.iter().any(|p| p[0] >= p[1]) {
            return Err(GeometryError::Degenerate {
                context: "command box",
            });
        }
        let origin = frame.origin().to_array();
        let axes = frame.axes().map(|a| a.as_vector().to_array());
        let mut vertices = Vec::with_capacity(8);
        for corner in CORNERS {
            let p = std::array::from_fn::<_, 3, _>(|axis| intervals[axis][corner[axis]]);
            let point = Point3::try_from(std::array::from_fn::<_, 3, _>(|i| {
                p[0].mul_add(
                    axes[0][i],
                    p[1].mul_add(axes[1][i], p[2].mul_add(axes[2][i], origin[i])),
                )
            }))?;
            vertices.push(BrepVertex::try_new(point, 0.)?);
        }
        let edges = EDGES
            .into_iter()
            .map(|v| {
                let line = LineSegment::try_new(
                    vertices[v[0]].point(),
                    vertices[v[1]].point(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                let length = line.length()?;
                let domain = parameter_extent(length, length);
                BrepEdge::try_new(v, line.try_reparameterized(0. ..=domain)?.to_nurbs()?, 0.)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut faces = Vec::with_capacity(6);
        let isos = [
            SurfaceIso::South,
            SurfaceIso::East,
            SurfaceIso::North,
            SurfaceIso::West,
        ];
        for (edge_indices, reversals) in FACES {
            let corners = std::array::from_fn::<_, 4, _>(|i| {
                edges[edge_indices[i]].vertices()[usize::from(reversals[i])]
            });
            let points = corners.map(|i| vertices[i].point());
            let u = parameter_extent(
                points[0].distance_to(points[1])?,
                points[2].distance_to(points[3])?,
            );
            let v = parameter_extent(
                points[0].distance_to(points[3])?,
                points[1].distance_to(points[2])?,
            );
            let surface =
                NurbsSurface::try_bilinear(points)?.try_reparameterized(0. ..=u, 0. ..=v)?;
            let uv = [
                Point2::try_new(0., 0.)?,
                Point2::try_new(u, 0.)?,
                Point2::try_new(u, v)?,
                Point2::try_new(0., v)?,
            ];
            let mut trims = Vec::with_capacity(4);
            for i in 0..4 {
                let extent = if i % 2 == 0 { u } else { v };
                let curve = NurbsCurve2::try_new(
                    1,
                    vec![uv[i], uv[(i + 1) % 4]],
                    vec![0., 0., extent, extent],
                )?;
                trims.push(BrepTrim::try_new(
                    [corners[i], corners[(i + 1) % 4]],
                    Some(edge_indices[i]),
                    reversals[i],
                    curve,
                    BrepTrimType::Mated,
                    isos[i],
                    [0., 0.],
                )?);
            }
            faces.push(BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame3 {
        Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn command_box_has_native_connectivity_domains_and_oriented_faces() {
        let box_brep =
            Brep::try_command_box(frame(), [[0., 2.], [0., 3.], [0., 4.]], Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(
            box_brep
                .vertices()
                .iter()
                .map(|v| v.point().to_array())
                .collect::<Vec<_>>(),
            vec![
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 3., 0.],
                [0., 3., 0.],
                [0., 0., 4.],
                [2., 0., 4.],
                [2., 3., 4.],
                [0., 3., 4.]
            ]
        );
        assert_eq!(
            box_brep
                .edges()
                .iter()
                .map(|e| e.vertices())
                .collect::<Vec<_>>(),
            EDGES
        );
        let dimensions = [[2., 4.], [3., 4.], [2., 4.], [3., 4.], [3., 2.], [2., 3.]];
        let normals = [
            [0., -1., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [-1., 0., 0.],
            [0., 0., -1.],
            [0., 0., 1.],
        ];
        for (index, face) in box_brep.faces().iter().enumerate() {
            let [u, v] = dimensions[index];
            assert_eq!(face.surface().domain_u(), 0. ..=u);
            assert_eq!(face.surface().domain_v(), 0. ..=v);
            assert_eq!(
                face.surface()
                    .normal_at(u / 2., v / 2.)
                    .unwrap()
                    .as_vector()
                    .to_array(),
                normals[index]
            );
            assert_eq!(
                face.loops()[0]
                    .trims()
                    .iter()
                    .map(|t| t.edge().unwrap())
                    .collect::<Vec<_>>(),
                FACES[index].0
            );
            for (side, trim) in face.loops()[0].trims().iter().enumerate() {
                assert_eq!(
                    trim.curve().domain(),
                    0. ..=if side % 2 == 0 { u } else { v }
                );
            }
        }
        assert!(box_brep.is_closed());
    }
    #[test]
    fn command_box_respects_rotated_frames_without_changing_normalized_boxes() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(10., 20., 30.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let bounds = [[0., 2.], [0., 3.], [0., 4.]];
        let native = Brep::try_command_box(frame, bounds, Tolerance::DEFAULT).unwrap();
        let generic = Brep::try_box(frame, bounds, Tolerance::DEFAULT).unwrap();
        assert_eq!(native.bounds(), generic.bounds());
        assert_eq!(generic.faces()[0].surface().domain_u(), 0. ..=1.);
        assert_eq!(native.bounds().min().to_array(), [10., 20., 30.]);
        assert_eq!(native.bounds().max().to_array(), [14., 22., 33.]);
    }
    #[test]
    fn tiny_command_box_uses_native_unit_parameter_fallback_and_rejects_invalid_intervals() {
        let box_brep = Brep::try_command_box(
            frame(),
            [[0., 1e-12]; 3],
            Tolerance::try_new(1e-24, 1e-12, 1e-10).unwrap(),
        )
        .unwrap();
        for face in box_brep.faces() {
            assert_eq!(face.surface().domain_u(), 0. ..=1.);
            assert_eq!(face.surface().domain_v(), 0. ..=1.);
        }
        for edge in box_brep.edges() {
            assert_eq!(edge.curve().domain(), 0. ..=1.);
        }
        for invalid in [[[0., 0.]; 3], [[1., 0.]; 3], [[0., f64::INFINITY]; 3]] {
            assert!(Brep::try_command_box(frame(), invalid, Tolerance::DEFAULT).is_err());
        }
    }
}
