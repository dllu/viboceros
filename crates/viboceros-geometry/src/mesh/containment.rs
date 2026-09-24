//! Point containment in an oriented, closed polygon mesh.

use super::TriangleMesh;
use crate::{BoundingBox3, GeometryError, Point3, Real, Tolerance};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolidPointLocation {
    Outside,
    Boundary,
    Inside,
}

/// A topology-validated mesh shell prepared for repeated point queries.
///
/// Boundary points are reported separately. A ray through an edge or vertex
/// is retried in other fixed directions, so face tessellation does not change
/// the answer for ordinary interior points.
pub struct MeshSolid<'a> {
    mesh: &'a TriangleMesh,
    bounds: BoundingBox3,
}

impl<'a> MeshSolid<'a> {
    pub fn from_closed_mesh(mesh: &'a TriangleMesh) -> Option<Self> {
        mesh.topology().is_solid().then(|| Self {
            mesh,
            bounds: mesh.bounds(),
        })
    }

    pub fn classify_point(
        &self,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<SolidPointLocation, GeometryError> {
        let position = point.to_array();
        let min = self.bounds.min().to_array();
        let max = self.bounds.max().to_array();
        let epsilon = tolerance.absolute();
        if (0..3).any(|axis| {
            (position[axis] < min[axis] && min[axis] - position[axis] > epsilon)
                || (position[axis] > max[axis] && position[axis] - max[axis] > epsilon)
        }) {
            return Ok(SolidPointLocation::Outside);
        }
        for face in 0..self.mesh.face_count() {
            let closest = self.mesh.closest_point_on_face(face, point)?;
            if point.distance_to(closest)? <= epsilon {
                return Ok(SolidPointLocation::Boundary);
            }
        }
        const DIRECTIONS: [[Real; 3]; 5] = [
            [1.0, 0.123_456_789, 0.314_159_265],
            [0.271_828_182, 1.0, 0.141_421_356],
            [0.173_205_081, 0.223_606_798, 1.0],
            [-0.618_033_989, 1.0, std::f64::consts::FRAC_1_SQRT_2],
            [1.0, -0.414_213_562, 0.577_215_665],
        ];
        for direction in DIRECTIONS {
            if let Some(crossings) = ray_crossings(self.mesh, point, direction) {
                return Ok(if crossings % 2 == 1 {
                    SolidPointLocation::Inside
                } else {
                    SolidPointLocation::Outside
                });
            }
        }
        // A deliberately aligned mesh can make every fixed ray graze an edge.
        // The oriented solid angle has no ray direction and resolves that case.
        Ok(
            if winding_angle(self.mesh, point).abs() > std::f64::consts::TAU {
                SolidPointLocation::Inside
            } else {
                SolidPointLocation::Outside
            },
        )
    }
}

fn dot(a: [Real; 3], b: [Real; 3]) -> Real {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn cross(a: [Real; 3], b: [Real; 3]) -> [Real; 3] {
    [
        a[1].mul_add(b[2], -a[2] * b[1]),
        a[2].mul_add(b[0], -a[0] * b[2]),
        a[0].mul_add(b[1], -a[1] * b[0]),
    ]
}

fn subtract(a: [Real; 3], b: [Real; 3]) -> [Real; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn relative_triangle(triangle: [Point3; 3], point: Point3) -> [[Real; 3]; 3] {
    let point = point.to_array();
    let vertices = triangle.map(Point3::to_array);
    let mut relative = vertices.map(|vertex| subtract(vertex, point));
    if relative.iter().flatten().any(|value| !value.is_finite()) {
        let scale = vertices
            .iter()
            .flatten()
            .chain(point.iter())
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        relative = vertices
            .map(|vertex| std::array::from_fn(|axis| vertex[axis] / scale - point[axis] / scale));
    }
    let scale = relative
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    if scale > 0.0 {
        relative.map(|vertex| vertex.map(|coordinate| coordinate / scale))
    } else {
        relative
    }
}

/// `None` means the ray touched an edge, vertex, or coplanar face.
fn ray_crossings(mesh: &TriangleMesh, point: Point3, direction: [Real; 3]) -> Option<usize> {
    const RELATIVE_EPSILON: Real = 64.0 * Real::EPSILON;
    let mut crossings = 0;
    for triangle in 0..mesh.triangles().len() {
        let [a, b, c] = relative_triangle(mesh.triangle_points(triangle)?, point);
        let ab = subtract(b, a);
        let ac = subtract(c, a);
        let normal = cross(ab, ac);
        let normal_length = dot(normal, normal).sqrt();
        let det = -dot(normal, direction);
        if det.abs() <= RELATIVE_EPSILON * normal_length {
            if dot(normal, a).abs() <= RELATIVE_EPSILON * normal_length {
                return None;
            }
            continue;
        }
        let h = cross(direction, ac);
        let u = -dot(a, h) / det;
        let q = cross(a.map(|value| -value), ab);
        let v = dot(direction, q) / det;
        let w = 1.0 - u - v;
        if u < -RELATIVE_EPSILON || v < -RELATIVE_EPSILON || w < -RELATIVE_EPSILON {
            continue;
        }
        let t = dot(ac, q) / det;
        if t < -RELATIVE_EPSILON {
            continue;
        }
        if t <= RELATIVE_EPSILON
            || u <= RELATIVE_EPSILON
            || v <= RELATIVE_EPSILON
            || w <= RELATIVE_EPSILON
        {
            return None;
        }
        crossings += 1;
    }
    Some(crossings)
}

fn winding_angle(mesh: &TriangleMesh, point: Point3) -> Real {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for triangle in 0..mesh.triangles().len() {
        let Some(triangle) = mesh.triangle_points(triangle) else {
            continue;
        };
        let [a, b, c] = relative_triangle(triangle, point).map(|vector| {
            let norm = dot(vector, vector).sqrt();
            vector.map(|value| value / norm)
        });
        let numerator = dot(a, cross(b, c));
        let denominator = 1.0 + dot(a, b) + dot(b, c) + dot(c, a);
        let angle = 2.0 * numerator.atan2(denominator);
        let next = sum + angle;
        if sum.abs() >= angle.abs() {
            correction += (sum - next) + angle;
        } else {
            correction += (angle - next) + sum;
        }
        sum = next;
    }
    sum + correction
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Brep, Frame3, NurbsSurface, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn box_mesh(origin: Point3) -> TriangleMesh {
        let frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        TriangleMesh::try_box_grid(
            frame,
            [[0.0, 2.0], [0.0, 2.0], [0.0, 2.0]],
            1,
            1,
            1,
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn box_interior_boundary_and_exterior_survive_unwelded_vertices() {
        let mesh = box_mesh(point(0.0, 0.0, 0.0));
        let solid = MeshSolid::from_closed_mesh(&mesh).unwrap();
        for (query, expected) in [
            (point(1.0, 1.0, 1.0), SolidPointLocation::Inside),
            (point(0.0, 1.0, 1.0), SolidPointLocation::Boundary),
            (point(0.0, 0.0, 1.0), SolidPointLocation::Boundary),
            (point(0.0, 0.0, 0.0), SolidPointLocation::Boundary),
            (point(3.0, 1.0, 1.0), SolidPointLocation::Outside),
            (point(-1.0, 1.0, 1.0), SolidPointLocation::Outside),
        ] {
            assert_eq!(
                solid.classify_point(query, Tolerance::DEFAULT).unwrap(),
                expected,
                "{query:?}"
            );
        }
    }

    #[test]
    fn grid_points_match_box_inequalities_after_translation_and_reversal() {
        let origin = point(10_000.0, -20_000.0, 30_000.0);
        let mesh = box_mesh(origin);
        for mesh in [&mesh, &mesh.reversed()] {
            let solid = MeshSolid::from_closed_mesh(mesh).unwrap();
            for x in -1..=5 {
                for y in -1..=5 {
                    for z in -1..=5 {
                        let local = [x, y, z].map(|value| value as Real * 0.5);
                        let query = point(
                            origin.x() + local[0],
                            origin.y() + local[1],
                            origin.z() + local[2],
                        );
                        let expected = if local.iter().any(|value| *value < 0.0 || *value > 2.0) {
                            SolidPointLocation::Outside
                        } else if local.iter().any(|value| *value == 0.0 || *value == 2.0) {
                            SolidPointLocation::Boundary
                        } else {
                            SolidPointLocation::Inside
                        };
                        assert_eq!(
                            solid.classify_point(query, Tolerance::DEFAULT).unwrap(),
                            expected,
                            "{local:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn open_mesh_is_not_a_solid() {
        let open = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(MeshSolid::from_closed_mesh(&open).is_none());
    }

    #[test]
    fn tessellated_brep_box_can_be_queried_as_a_solid() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 2.0], [0.0, 2.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mesh = brep.tessellate(16, Tolerance::DEFAULT).unwrap();
        let solid = MeshSolid::from_closed_mesh(&mesh).unwrap();
        assert_eq!(
            solid
                .classify_point(point(1.0, 1.0, 1.0), Tolerance::DEFAULT)
                .unwrap(),
            SolidPointLocation::Inside
        );
    }

    #[test]
    fn closed_nurbs_sphere_and_torus_have_watertight_tessellations() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0)
            .unwrap()
            .tessellate(16, Tolerance::DEFAULT)
            .unwrap();
        let torus = NurbsSurface::try_torus(frame, 3.0, 1.0)
            .unwrap()
            .tessellate(16, Tolerance::DEFAULT)
            .unwrap();
        for (mesh, inside, outside) in [
            (&sphere, point(0.0, 0.0, 0.0), point(3.0, 0.0, 0.0)),
            (&torus, point(3.0, 0.0, 0.0), point(0.0, 0.0, 0.0)),
        ] {
            let solid = MeshSolid::from_closed_mesh(mesh).unwrap();
            assert_eq!(
                solid.classify_point(inside, Tolerance::DEFAULT).unwrap(),
                SolidPointLocation::Inside
            );
            assert_eq!(
                solid.classify_point(outside, Tolerance::DEFAULT).unwrap(),
                SolidPointLocation::Outside
            );
        }
    }
}
