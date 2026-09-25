use super::*;

/// Threshold used to identify a nonplanar quad. Values are model units and radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NonPlanarQuadCriterion {
    Distance(Real),
    Angle(Real),
    Both { distance: Real, angle: Real },
}

/// Diagonal selection for nonplanar quad triangulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuadSplitMethod {
    ShortestDiagonal,
    LongestDiagonal,
    MinimizeArea,
    MaximizeArea,
    MinimumAngle,
    MaximumAngle,
}

impl TriangleMesh {
    /// Splits only quads meeting the selected nonplanarity threshold.
    /// The distance is from D to the ABC plane; the angle compares the two
    /// triangle normals for the chosen diagonal. Equality passes either test.
    pub fn triangulate_nonplanar_quads(
        &self,
        criterion: NonPlanarQuadCriterion,
        method: QuadSplitMethod,
        tolerance: Tolerance,
    ) -> Result<(Self, usize), GeometryError> {
        let valid = match criterion {
            NonPlanarQuadCriterion::Distance(distance) => distance.is_finite() && distance > 0.,
            NonPlanarQuadCriterion::Angle(angle) => {
                angle.is_finite() && (0. ..=std::f64::consts::PI).contains(&angle)
            }
            NonPlanarQuadCriterion::Both { distance, angle } => {
                distance.is_finite()
                    && distance > 0.
                    && angle.is_finite()
                    && (0. ..=std::f64::consts::PI).contains(&angle)
            }
        };
        if !valid {
            return Err(GeometryError::InvalidTolerance);
        }
        self.triangulate_selected_quads(tolerance, |vertices, quad| {
            let points = quad.map(|index| vertices[index as usize]);
            let ac = [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]];
            let bd = [[quad[0], quad[1], quad[3]], [quad[1], quad[2], quad[3]]];
            let choose_ac = match method {
                QuadSplitMethod::ShortestDiagonal => mass_triangles::split(vertices, quad) == ac,
                QuadSplitMethod::LongestDiagonal => mass_triangles::split(vertices, quad) != ac,
                QuadSplitMethod::MinimizeArea | QuadSplitMethod::MaximizeArea => {
                    let ac_area = triangle_area(vertices, ac[0])? + triangle_area(vertices, ac[1])?;
                    let bd_area = triangle_area(vertices, bd[0])? + triangle_area(vertices, bd[1])?;
                    if method == QuadSplitMethod::MinimizeArea {
                        ac_area <= bd_area
                    } else {
                        ac_area >= bd_area
                    }
                }
                QuadSplitMethod::MinimumAngle | QuadSplitMethod::MaximumAngle => {
                    let ac_angle = normal_angle(vertices, ac)?;
                    let bd_angle = normal_angle(vertices, bd)?;
                    if method == QuadSplitMethod::MinimumAngle {
                        ac_angle <= bd_angle
                    } else {
                        ac_angle >= bd_angle
                    }
                }
            };
            let triangles = if choose_ac { ac } else { bd };
            let distance = || -> Result<Real, GeometryError> {
                let ab = points[0].direction_to(points[1])?.as_vector();
                let ac = points[0].direction_to(points[2])?.as_vector();
                let normal = ab.cross(ac)?.normalized_nonzero()?.as_vector();
                Ok(normal.dot_point_difference(points[3], points[0]).abs())
            };
            let passes = match criterion {
                NonPlanarQuadCriterion::Distance(limit) => distance()? >= limit,
                NonPlanarQuadCriterion::Angle(limit) => normal_angle(vertices, triangles)? >= limit,
                NonPlanarQuadCriterion::Both {
                    distance: limit,
                    angle,
                } => distance()? >= limit && normal_angle(vertices, triangles)? >= angle,
            };
            Ok(passes.then_some(triangles))
        })
    }
}

fn triangle_normal(
    vertices: &[Point3],
    triangle: [u32; 3],
) -> Result<crate::Vector3, GeometryError> {
    let [a, b, c] = triangle.map(|index| vertices[index as usize]);
    let ab = a.direction_to(b)?.as_vector();
    let ac = a.direction_to(c)?.as_vector();
    Ok(ab.cross(ac)?.normalized_nonzero()?.as_vector())
}

fn normal_angle(vertices: &[Point3], triangles: [[u32; 3]; 2]) -> Result<Real, GeometryError> {
    triangle_normal(vertices, triangles[0])?.angle_to(triangle_normal(vertices, triangles[1])?)
}

fn triangle_area(vertices: &[Point3], triangle: [u32; 3]) -> Result<Real, GeometryError> {
    let [a, b, c] = triangle.map(|index| vertices[index as usize]);
    a.vector_to(b)?.half_cross_length(a.vector_to(c)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_and_split_methods_keep_planar_quads_and_select_nonplanar_ones() {
        let points = [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [0., 1., 0.4],
            [3., 0., 0.],
            [5., 0., 0.],
            [5., 1., 0.],
            [3., 1., 0.],
        ]
        .map(|point| Point3::try_from(point).unwrap());
        let mesh = TriangleMesh::try_new_faces(
            points.to_vec(),
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_ngons(vec![
            MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0]),
            MeshNgon::from_parts(vec![4, 5, 6, 7], vec![1]),
        ])
        .unwrap();
        let (split, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Distance(0.3),
                QuadSplitMethod::ShortestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(split.face_count(), 3);
        assert!(matches!(split.faces()[1], MeshFace::Quad(_)));
        assert_eq!(split.ngons()[0].faces(), &[0, 2]);
        assert_eq!(split.ngons()[1].faces(), &[1]);
        let (unchanged, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Distance(0.5),
                QuadSplitMethod::LongestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(unchanged, mesh);
        for method in [
            QuadSplitMethod::MinimizeArea,
            QuadSplitMethod::MaximizeArea,
            QuadSplitMethod::MinimumAngle,
            QuadSplitMethod::MaximumAngle,
        ] {
            let (result, count) = mesh
                .triangulate_nonplanar_quads(
                    NonPlanarQuadCriterion::Distance(0.3),
                    method,
                    Tolerance::DEFAULT,
                )
                .unwrap();
            assert_eq!(count, 1);
            assert_eq!(result.ngons()[0].faces(), &[0, 2]);
        }
        let (long, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Angle(0.1),
                QuadSplitMethod::LongestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_ne!(split.faces()[0], long.faces()[0]);
        let (_, count) = mesh
            .triangulate_nonplanar_quads(
                NonPlanarQuadCriterion::Both {
                    distance: 0.3,
                    angle: 1.0,
                },
                QuadSplitMethod::ShortestDiagonal,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
