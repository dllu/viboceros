//! Point containment in an oriented, closed polygon mesh.

use super::TriangleMesh;
use crate::{BoundingBox3, GeometryError, Point3, Real, Tolerance};
use std::ops::Range;

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
    faces: Vec<FaceRecord>,
    nodes: Vec<Node>,
}

struct FaceRecord {
    index: usize,
    min: [Real; 3],
    max: [Real; 3],
    center: [Real; 3],
}

struct Node {
    min: [Real; 3],
    max: [Real; 3],
    range: Range<usize>,
    children: Option<[usize; 2]>,
}

impl<'a> MeshSolid<'a> {
    pub fn from_closed_mesh(mesh: &'a TriangleMesh) -> Option<Self> {
        if !mesh.topology().is_solid() {
            return None;
        }
        let faces = mesh
            .faces()
            .iter()
            .enumerate()
            .map(|(index, face)| {
                let bounds = BoundingBox3::from_points(
                    face.indices()
                        .iter()
                        .map(|&vertex| mesh.vertices()[vertex as usize]),
                )
                .expect("validated mesh faces contain finite points");
                FaceRecord {
                    index,
                    min: bounds.min().to_array(),
                    max: bounds.max().to_array(),
                    center: bounds.center().expect("finite bounds center").to_array(),
                }
            })
            .collect();
        let mut solid = Self {
            mesh,
            bounds: mesh.bounds(),
            faces,
            nodes: Vec::new(),
        };
        if !solid.faces.is_empty() {
            solid.build(0..solid.faces.len());
        }
        Some(solid)
    }

    fn build(&mut self, range: Range<usize>) -> usize {
        let mut min = [Real::INFINITY; 3];
        let mut max = [Real::NEG_INFINITY; 3];
        for face in &self.faces[range.clone()] {
            for axis in 0..3 {
                min[axis] = min[axis].min(face.min[axis]);
                max[axis] = max[axis].max(face.max[axis]);
            }
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            min,
            max,
            range: range.clone(),
            children: None,
        });
        if range.len() > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b])))
                .expect("three axes");
            let middle = range.start + range.len() / 2;
            self.faces[range.clone()].select_nth_unstable_by(range.len() / 2, |a, b| {
                a.center[axis].total_cmp(&b.center[axis])
            });
            let children = [
                self.build(range.start..middle),
                self.build(middle..range.end),
            ];
            self.nodes[node].children = Some(children);
        }
        node
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
        if outside_bounds(min, max, position, epsilon) {
            return Ok(SolidPointLocation::Outside);
        }
        if !self.nodes.is_empty() && self.boundary_hit(0, point, epsilon)? {
            return Ok(SolidPointLocation::Boundary);
        }
        const DIRECTIONS: [[Real; 3]; 5] = [
            [1.0, 0.123_456_789, 0.314_159_265],
            [0.271_828_182, 1.0, 0.141_421_356],
            [0.173_205_081, 0.223_606_798, 1.0],
            [-0.618_033_989, 1.0, std::f64::consts::FRAC_1_SQRT_2],
            [1.0, -0.414_213_562, 0.577_215_665],
        ];
        for direction in DIRECTIONS {
            if let Some(crossings) = self.ray_crossings(point, direction) {
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

    fn boundary_hit(
        &self,
        index: usize,
        point: Point3,
        epsilon: Real,
    ) -> Result<bool, GeometryError> {
        let node = &self.nodes[index];
        if outside_bounds(node.min, node.max, point.to_array(), epsilon) {
            return Ok(false);
        }
        if let Some(children) = node.children {
            return Ok(self.boundary_hit(children[0], point, epsilon)?
                || self.boundary_hit(children[1], point, epsilon)?);
        }
        for face in &self.faces[node.range.clone()] {
            let closest = self.mesh.closest_point_on_face(face.index, point)?;
            if point.distance_to(closest)? <= epsilon {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `None` means the ray touched an edge, vertex, or coplanar face.
    fn ray_crossings(&self, point: Point3, direction: [Real; 3]) -> Option<usize> {
        let mut crossings = 0;
        if !self.nodes.is_empty() && !self.visit_ray(0, point, direction, &mut crossings) {
            return None;
        }
        Some(crossings)
    }

    fn visit_ray(
        &self,
        index: usize,
        point: Point3,
        direction: [Real; 3],
        crossings: &mut usize,
    ) -> bool {
        let node = &self.nodes[index];
        if ray_misses_bounds(node.min, node.max, point.to_array(), direction) {
            return true;
        }
        if let Some(children) = node.children {
            return self.visit_ray(children[0], point, direction, crossings)
                && self.visit_ray(children[1], point, direction, crossings);
        }
        for face in &self.faces[node.range.clone()] {
            let indices = self.mesh.faces()[face.index].indices();
            for triangle in [[0, 1, 2], [0, 2, 3]] {
                if triangle[2] >= indices.len() {
                    continue;
                }
                let vertices =
                    triangle.map(|corner| self.mesh.vertices()[indices[corner] as usize]);
                match ray_triangle_crossing(vertices, point, direction) {
                    RayCrossing::Miss => {}
                    RayCrossing::Hit => *crossings += 1,
                    RayCrossing::Ambiguous => return false,
                }
            }
        }
        true
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

fn outside_bounds(min: [Real; 3], max: [Real; 3], point: [Real; 3], epsilon: Real) -> bool {
    (0..3).any(|axis| {
        (point[axis] < min[axis] && min[axis] - point[axis] > epsilon)
            || (point[axis] > max[axis] && point[axis] - max[axis] > epsilon)
    })
}

fn ray_misses_bounds(
    min: [Real; 3],
    max: [Real; 3],
    point: [Real; 3],
    direction: [Real; 3],
) -> bool {
    let mut near = 0.0_f64;
    let mut far = Real::INFINITY;
    for axis in 0..3 {
        let low = min[axis].next_down();
        let high = max[axis].next_up();
        if direction[axis] == 0.0 {
            if point[axis] < low || point[axis] > high {
                return true;
            }
            continue;
        }
        let first = (low - point[axis]) / direction[axis];
        let second = (high - point[axis]) / direction[axis];
        if !first.is_finite() || !second.is_finite() {
            return false;
        }
        near = near.max(first.min(second));
        far = far.min(first.max(second));
        let slack = 64.0 * Real::EPSILON * near.abs().max(far.abs()).max(1.0);
        if near > far && near - far > slack {
            return true;
        }
    }
    false
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

enum RayCrossing {
    Miss,
    Hit,
    Ambiguous,
}

fn ray_triangle_crossing(
    triangle: [Point3; 3],
    point: Point3,
    direction: [Real; 3],
) -> RayCrossing {
    const RELATIVE_EPSILON: Real = 64.0 * Real::EPSILON;
    let [a, b, c] = relative_triangle(triangle, point);
    let ab = subtract(b, a);
    let ac = subtract(c, a);
    let normal = cross(ab, ac);
    let normal_length = dot(normal, normal).sqrt();
    let det = -dot(normal, direction);
    if det.abs() <= RELATIVE_EPSILON * normal_length {
        if dot(normal, a).abs() <= RELATIVE_EPSILON * normal_length {
            return RayCrossing::Ambiguous;
        }
        return RayCrossing::Miss;
    }
    let h = cross(direction, ac);
    let u = -dot(a, h) / det;
    let q = cross(a.map(|value| -value), ab);
    let v = dot(direction, q) / det;
    let w = 1.0 - u - v;
    if u < -RELATIVE_EPSILON || v < -RELATIVE_EPSILON || w < -RELATIVE_EPSILON {
        return RayCrossing::Miss;
    }
    let t = dot(ac, q) / det;
    if t < -RELATIVE_EPSILON {
        return RayCrossing::Miss;
    }
    if t <= RELATIVE_EPSILON
        || u <= RELATIVE_EPSILON
        || v <= RELATIVE_EPSILON
        || w <= RELATIVE_EPSILON
    {
        return RayCrossing::Ambiguous;
    }
    RayCrossing::Hit
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

    fn candidate_faces(
        solid: &MeshSolid<'_>,
        index: usize,
        intersects: &impl Fn(&Node) -> bool,
    ) -> usize {
        let node = &solid.nodes[index];
        if !intersects(node) {
            return 0;
        }
        match node.children {
            Some([left, right]) => {
                candidate_faces(solid, left, intersects) + candidate_faces(solid, right, intersects)
            }
            None => node.range.len(),
        }
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
    fn dense_box_queries_visit_local_faces() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mesh = TriangleMesh::try_box_grid(
            frame,
            [[0.0, 2.0], [0.0, 2.0], [0.0, 2.0]],
            64,
            64,
            1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let solid = MeshSolid::from_closed_mesh(&mesh).unwrap();
        assert!(mesh.faces().len() > 8_000);
        for x in -1..=9 {
            for y in -1..=9 {
                for z in [-0.5, 0.0, 0.5, 1.0, 2.0, 2.5] {
                    let [x, y] = [x, y].map(|coordinate| coordinate as Real / 4.0);
                    let query = point(x, y, z);
                    let expected = if [x, y, z]
                        .iter()
                        .any(|&coordinate| !(0.0..=2.0).contains(&coordinate))
                    {
                        SolidPointLocation::Outside
                    } else if [x, y, z]
                        .iter()
                        .any(|&coordinate| coordinate == 0.0 || coordinate == 2.0)
                    {
                        SolidPointLocation::Boundary
                    } else {
                        SolidPointLocation::Inside
                    };
                    assert_eq!(
                        solid.classify_point(query, Tolerance::DEFAULT).unwrap(),
                        expected,
                        "{query:?}"
                    );
                }
            }
        }

        let query = point(1.013, 1.017, 0.0);
        let boundary_candidates = candidate_faces(&solid, 0, &|node| {
            !outside_bounds(
                node.min,
                node.max,
                query.to_array(),
                Tolerance::DEFAULT.absolute(),
            )
        });
        assert!(boundary_candidates < 128, "{boundary_candidates}");
        let direction = [1.0, 0.123_456_789, 0.314_159_265];
        let ray_candidates = candidate_faces(&solid, 0, &|node| {
            !ray_misses_bounds(
                node.min,
                node.max,
                point(1.013, 1.017, 1.0).to_array(),
                direction,
            )
        });
        assert!(ray_candidates < 128, "{ray_candidates}");
    }

    #[test]
    fn bounds_tree_preserves_ray_crossings_on_curved_shells() {
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
        let directions = [
            [1.0, 0.123_456_789, 0.314_159_265],
            [0.271_828_182, 1.0, 0.141_421_356],
            [0.173_205_081, 0.223_606_798, 1.0],
        ];
        for mesh in [&sphere, &torus] {
            let solid = MeshSolid::from_closed_mesh(mesh).unwrap();
            for sample in 0..64 {
                let query = point(
                    ((sample * 17) % 71) as Real / 8.0 - 4.0,
                    ((sample * 29) % 67) as Real / 8.0 - 4.0,
                    ((sample * 37) % 61) as Real / 8.0 - 3.0,
                );
                for direction in directions {
                    let mut brute_crossings = 0;
                    let mut brute_ambiguous = false;
                    for triangle in 0..mesh.triangles().len() {
                        match ray_triangle_crossing(
                            mesh.triangle_points(triangle).unwrap(),
                            query,
                            direction,
                        ) {
                            RayCrossing::Miss => {}
                            RayCrossing::Hit => brute_crossings += 1,
                            RayCrossing::Ambiguous => {
                                brute_ambiguous = true;
                                break;
                            }
                        }
                    }
                    assert_eq!(
                        solid.ray_crossings(query, direction),
                        (!brute_ambiguous).then_some(brute_crossings),
                        "{query:?}, {direction:?}"
                    );
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
