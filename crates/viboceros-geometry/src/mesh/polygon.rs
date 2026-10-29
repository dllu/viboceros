//! Shared normalized projection, constrained triangulation and polygon predicates.
//! Mesh edits own their face layout, winding and channel policies.
use crate::{GeometryError, Point3, Real, require_finite};
use spade::{
    ConstrainedDelaunayTriangulation, HasPosition, Point2 as TriangulationPoint2, Triangulation,
};
use std::{collections::VecDeque, ops::Range};

#[derive(Clone, Copy, Debug)]
struct MeshHoleTriangulationVertex {
    position: TriangulationPoint2<Real>,
    source_index: usize,
}

impl HasPosition for MeshHoleTriangulationVertex {
    type Scalar = Real;

    fn position(&self) -> TriangulationPoint2<Self::Scalar> {
        self.position
    }
}

pub(super) fn triangulate_projected_mesh_hole(
    projected: &[[Real; 2]],
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    let loop_range = 0..projected.len();
    triangulate_projected_mesh_region(projected, std::slice::from_ref(&loop_range))
}

/// The first loop is the outer boundary; later loops are disjoint inner holes.
/// All coordinates share one normalized planar frame.
pub(super) fn triangulate_projected_mesh_region(
    projected: &[[Real; 2]],
    loops: &[Range<usize>],
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    if projected.len() > u32::MAX as usize {
        return Err(GeometryError::TooManyMeshVertices);
    }
    if loops.is_empty()
        || loops.iter().any(|range| range.len() < 3)
        || loops.first().is_none_or(|range| range.start != 0)
        || loops
            .last()
            .is_none_or(|range| range.end != projected.len())
        || loops.windows(2).any(|pair| pair[0].end != pair[1].start)
    {
        return Ok(None);
    }
    let loop_points = loops
        .iter()
        .map(|range| &projected[range.clone()])
        .collect::<Vec<_>>();
    let doubled_area = projected_polygon_doubled_area(loop_points[0]).abs()
        - loop_points[1..]
            .iter()
            .map(|points| projected_polygon_doubled_area(points).abs())
            .sum::<Real>();
    let epsilon = 64.0 * Real::EPSILON * projected.len() as Real;
    if !doubled_area.is_finite() || doubled_area <= epsilon {
        return Ok(None);
    }

    let vertices = projected
        .iter()
        .enumerate()
        .map(|(source_index, point)| MeshHoleTriangulationVertex {
            position: TriangulationPoint2::new(point[0], point[1]),
            source_index,
        })
        .collect::<Vec<_>>();
    let mut triangulation =
        match ConstrainedDelaunayTriangulation::<MeshHoleTriangulationVertex>::bulk_load(vertices) {
            Ok(triangulation) => triangulation,
            Err(_) => return Ok(None),
        };
    if triangulation.num_vertices() != projected.len() {
        return Ok(None);
    }
    let mut handles = vec![None; projected.len()];
    for vertex in triangulation.vertices() {
        handles[vertex.data().source_index] = Some(vertex.fix());
    }
    for range in loops {
        for source in range.clone() {
            let Some(from) = handles[source] else {
                return Ok(None);
            };
            let Some(to) = handles[if source + 1 == range.end {
                range.start
            } else {
                source + 1
            }] else {
                return Ok(None);
            };
            let before = triangulation.num_constraints();
            if triangulation.try_add_constraint(from, to).is_empty()
                || triangulation.num_constraints() != before + 1
            {
                return Ok(None);
            }
        }
    }

    let expected_triangle_count = projected
        .len()
        .checked_add(loops.len().saturating_sub(1).saturating_mul(2))
        .and_then(|count| count.checked_sub(2))
        .ok_or(GeometryError::TooManyMeshFaces)?;
    let mut triangles = Vec::new();
    triangles
        .try_reserve(expected_triangle_count)
        .map_err(|_| GeometryError::TooManyMeshFaces)?;
    let mut actual_area = 0.0;
    let mut area_correction = 0.0;
    // Classify the dual graph once. Crossing a boundary toggles membership;
    // winding does not matter. This preserves narrow strips that centroid
    // tests with a polygon-wide epsilon can mistake for hole boundaries.
    // Count and area audits below still require one outer loop with disjoint,
    // unnested holes, rejecting islands and misplaced inner loops.
    let mut inside = vec![None; triangulation.num_all_faces()];
    let outer = triangulation.outer_face();
    inside[outer.index()] = Some(false);
    let mut queue = VecDeque::from([outer]);
    while let Some(face) = queue.pop_front() {
        let current_inside = inside[face.index()].unwrap();
        let Some(first) = face.adjacent_edge() else {
            return Ok(None);
        };
        let mut edge = first;
        loop {
            let neighbor = edge.rev().face();
            let expected = current_inside ^ edge.is_constraint_edge();
            match inside[neighbor.index()] {
                Some(actual) if actual != expected => return Ok(None),
                Some(_) => {}
                None => {
                    inside[neighbor.index()] = Some(expected);
                    queue.push_back(neighbor);
                }
            }
            edge = edge.next();
            if edge.fix() == first.fix() {
                break;
            }
        }
    }
    for face in triangulation.inner_faces() {
        if inside[face.index()] != Some(true) {
            continue;
        }
        let face_vertices = face.vertices();
        let face_points = face_vertices.map(|vertex| {
            let point = vertex.data().position;
            [point.x, point.y]
        });
        let triangle_area = mesh_hole_cross(face_points[0], face_points[1], face_points[2]);
        // Face orientation is local: the polygon-wide area tolerance grows
        // with vertex count and would discard valid ears of dense boundaries.
        if triangle_area <= 0.0 {
            return Ok(None);
        }
        compensated_mesh_hole_sum(&mut actual_area, &mut area_correction, triangle_area);
        let mut triangle = [0_u32; 3];
        for (target, vertex) in triangle.iter_mut().zip(face_vertices) {
            *target = u32::try_from(vertex.data().source_index)
                .map_err(|_| GeometryError::TooManyMeshVertices)?;
        }
        triangles.push(triangle);
    }
    if triangles.len() != expected_triangle_count {
        return Ok(None);
    }
    let actual_area = actual_area + area_correction;
    let area_tolerance = 4096.0 * Real::EPSILON * projected.len() as Real;
    if (actual_area - doubled_area).abs() > area_tolerance {
        return Ok(None);
    }
    Ok(Some(triangles))
}

pub(super) fn project_mesh_hole_boundary(
    points: &[Point3],
) -> Result<Option<Vec<[Real; 2]>>, GeometryError> {
    let Some(origin) = points.first() else {
        return Ok(None);
    };
    let direct = points
        .iter()
        .map(|point| {
            [
                point.x() - origin.x(),
                point.y() - origin.y(),
                point.z() - origin.z(),
            ]
        })
        .collect::<Vec<_>>();
    let mut relative = if direct.iter().flatten().all(|value| value.is_finite()) {
        direct
    } else {
        let global_scale = points
            .iter()
            .flat_map(|point| [point.x().abs(), point.y().abs(), point.z().abs()])
            .fold(0.0, Real::max);
        if global_scale == 0.0 {
            return Ok(None);
        }
        let scaled_origin = [
            origin.x() / global_scale,
            origin.y() / global_scale,
            origin.z() / global_scale,
        ];
        points
            .iter()
            .map(|point| {
                [
                    point.x() / global_scale - scaled_origin[0],
                    point.y() / global_scale - scaled_origin[1],
                    point.z() / global_scale - scaled_origin[2],
                ]
            })
            .collect()
    };
    let scale = relative
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    for point in &mut relative {
        for coordinate in point {
            *coordinate /= scale;
        }
    }

    let mut normal_sum = [0.0; 3];
    let mut normal_correction = [0.0; 3];
    for index in 0..relative.len() {
        let first = relative[index];
        let second = relative[(index + 1) % relative.len()];
        let cross = [
            first[1].mul_add(second[2], -first[2] * second[1]),
            first[2].mul_add(second[0], -first[0] * second[2]),
            first[0].mul_add(second[1], -first[1] * second[0]),
        ];
        for coordinate in 0..3 {
            compensated_mesh_hole_sum(
                &mut normal_sum[coordinate],
                &mut normal_correction[coordinate],
                cross[coordinate],
            );
        }
    }
    let normal: [Real; 3] =
        std::array::from_fn(|coordinate| normal_sum[coordinate] + normal_correction[coordinate]);
    let normal_scale = normal.iter().map(|value| value.abs()).fold(0.0, Real::max);
    if normal_scale <= 64.0 * Real::EPSILON * points.len() as Real {
        return Ok(None);
    }
    let scaled_normal = normal.map(|value| value / normal_scale);
    let normal_length = scaled_normal
        .iter()
        .map(|value| value * value)
        .sum::<Real>()
        .sqrt();
    let normal = scaled_normal.map(|value| value / normal_length);

    let mut tangent = None;
    for index in 0..relative.len() {
        let first = relative[index];
        let second = relative[(index + 1) % relative.len()];
        let edge =
            std::array::from_fn::<_, 3, _>(|coordinate| second[coordinate] - first[coordinate]);
        let normal_component =
            edge[0].mul_add(normal[0], edge[1].mul_add(normal[1], edge[2] * normal[2]));
        let planar = std::array::from_fn::<_, 3, _>(|coordinate| {
            normal_component.mul_add(-normal[coordinate], edge[coordinate])
        });
        let squared_length = planar.iter().map(|value| value * value).sum::<Real>();
        if tangent.is_none_or(|(_, best_length)| squared_length > best_length) {
            tangent = Some((planar, squared_length));
        }
    }
    let Some((tangent, tangent_squared_length)) = tangent else {
        return Ok(None);
    };
    if tangent_squared_length <= Real::MIN_POSITIVE {
        return Ok(None);
    }
    let tangent_length = tangent_squared_length.sqrt();
    let x_axis = tangent.map(|value| value / tangent_length);
    let y_axis = [
        normal[1].mul_add(x_axis[2], -normal[2] * x_axis[1]),
        normal[2].mul_add(x_axis[0], -normal[0] * x_axis[2]),
        normal[0].mul_add(x_axis[1], -normal[1] * x_axis[0]),
    ];
    let projected = relative
        .into_iter()
        .map(|point| {
            [
                point[0].mul_add(x_axis[0], point[1].mul_add(x_axis[1], point[2] * x_axis[2])),
                point[0].mul_add(y_axis[0], point[1].mul_add(y_axis[1], point[2] * y_axis[2])),
            ]
        })
        .collect::<Vec<_>>();
    require_finite(projected.iter().flatten().copied(), "mesh hole projection")?;
    Ok(Some(projected))
}

pub(super) fn projected_polygon_doubled_area(points: &[[Real; 2]]) -> Real {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        compensated_mesh_hole_sum(
            &mut sum,
            &mut correction,
            first[0].mul_add(second[1], -first[1] * second[0]),
        );
    }
    sum + correction
}

pub(super) fn mesh_hole_cross(first: [Real; 2], second: [Real; 2], third: [Real; 2]) -> Real {
    let first_edge = [second[0] - first[0], second[1] - first[1]];
    let second_edge = [third[0] - first[0], third[1] - first[1]];
    first_edge[0].mul_add(second_edge[1], -first_edge[1] * second_edge[0])
}

pub(crate) fn point_in_mesh_hole_polygon(
    point: [Real; 2],
    polygon: &[[Real; 2]],
    epsilon: Real,
) -> bool {
    let mut winding = 0_i64;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let cross = mesh_hole_cross(start, end, point);
        if cross.abs() <= epsilon
            && point[0] >= start[0].min(end[0]) - epsilon
            && point[0] <= start[0].max(end[0]) + epsilon
            && point[1] >= start[1].min(end[1]) - epsilon
            && point[1] <= start[1].max(end[1]) + epsilon
        {
            return true;
        }
        if start[1] <= point[1] {
            if end[1] > point[1] && cross > epsilon {
                winding += 1;
            }
        } else if end[1] <= point[1] && cross < -epsilon {
            winding -= 1;
        }
    }
    winding != 0
}

fn compensated_mesh_hole_sum(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

#[cfg(test)]
mod tests;
