//! Indexed control loops, nested planar regions and shared constrained drafts.
use super::*;
use crate::exact_scalar::{Rational, rational};
use crate::{
    PointProjection3,
    mesh::{
        PolygonProjection, mesh_hole_cross, point_in_mesh_hole_polygon,
        projected_polygon_doubled_area, triangulate_projected_mesh_region,
    },
};
use num_traits::Zero;
use rstar::{AABB, RTree, RTreeObject};
use std::collections::BTreeSet;

const WORK_LIMIT: usize = 4_000_000;
fn charge(work: &mut usize, count: usize) -> Result<(), GeometryError> {
    *work = work
        .checked_sub(count)
        .ok_or(GeometryError::SubDCapResourceLimit {
            maximum: WORK_LIMIT,
        })?;
    Ok(())
}

pub(super) struct Draft {
    pub points: Vec<Point3>,
    pub faces: Vec<Vec<u32>>,
    pub boundary_edges: Vec<usize>,
    pub boundaries: usize,
}
struct Boundary {
    vertices: Vec<u32>,
    edges: Vec<usize>,
    points: Vec<Point3>,
    bounds: BoundingBox3,
    plane: PointProjection3,
    frame: PolygonProjection,
    projected: Vec<[f64; 2]>,
    area: Rational,
}
struct BoxEntry {
    index: usize,
    bounds: AABB<[f64; 3]>,
}
impl RTreeObject for BoxEntry {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}
fn envelope(bounds: BoundingBox3) -> AABB<[f64; 3]> {
    AABB::from_corners(bounds.min().to_array(), bounds.max().to_array())
}
fn on_plane(
    points: &[Point3],
    plane: &PointProjection3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for &point in points {
        if point.distance_to(plane.project(point)?)? > tolerance.absolute() {
            return Ok(false);
        }
    }
    Ok(true)
}
fn area_squared(points: &[Point3]) -> Rational {
    let mut sum: [Rational; 3] = std::array::from_fn(|_| Rational::zero());
    for i in 0..points.len() {
        let a = points[i].to_array().map(rational);
        let b = points[(i + 1) % points.len()].to_array().map(rational);
        for (k, s) in sum.iter_mut().enumerate() {
            let (u, v) = ((k + 1) % 3, (k + 2) % 3);
            *s += &a[u] * &b[v] - &a[v] * &b[u];
        }
    }
    sum.iter().map(|x| x * x).sum()
}

pub(super) fn prepare(
    source: &SubD,
    tolerance: Tolerance,
    triangles: bool,
) -> Result<Draft, GeometryError> {
    prepare_with_budget(source, tolerance, triangles, WORK_LIMIT)
}

pub(super) fn prepare_with_budget(
    source: &SubD,
    tolerance: Tolerance,
    triangles: bool,
    mut work: usize,
) -> Result<Draft, GeometryError> {
    let edges = source
        .edge_face_counts()
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 1).then_some(i))
        .collect::<Vec<_>>();
    let mut loops = Vec::new();
    for chain in source.try_edge_chains(&edges)? {
        if !chain.is_closed() || chain.vertices.len() < 4 {
            continue;
        }
        let vertices = chain.vertices[..chain.vertices.len() - 1]
            .iter()
            .map(|&i| i as u32)
            .collect::<Vec<_>>();
        if vertices.iter().collect::<BTreeSet<_>>().len() != vertices.len() {
            continue;
        }
        let points = vertices
            .iter()
            .map(|&i| source.control_points()[i as usize])
            .collect::<Vec<_>>();
        charge(&mut work, points.len())?;
        let plane = PointProjection3::onto_best_fit_plane(&points)?;
        if !on_plane(&points, &plane, tolerance)? {
            continue;
        }
        let Some(frame) = PolygonProjection::new(&points)? else {
            continue;
        };
        let projected = points
            .iter()
            .map(|&p| frame.project(p))
            .collect::<Result<Vec<_>, _>>()?;
        let area = area_squared(&points);
        let bounds = BoundingBox3::from_points(points.iter().copied())?;
        loops.push(Boundary {
            vertices,
            edges: chain.edges.iter().map(|e| e.index).collect(),
            points,
            bounds,
            plane,
            frame,
            projected,
            area,
        });
    }
    let tree = RTree::bulk_load(
        loops
            .iter()
            .enumerate()
            .map(|(index, b)| BoxEntry {
                index,
                bounds: envelope(b.bounds),
            })
            .collect(),
    );
    let mut parent = vec![None; loops.len()];
    let mut ambiguous = vec![false; loops.len()];
    for (i, inner) in loops.iter().enumerate() {
        for entry in tree.locate_in_envelope_intersecting(envelope(inner.bounds)) {
            let j = entry.index;
            let outer = &loops[j];
            if i == j {
                continue;
            }
            charge(
                &mut work,
                inner.points.len().saturating_mul(outer.points.len()),
            )?;
            if !on_plane(&inner.points, &outer.plane, tolerance)? {
                continue;
            }
            if outer.area <= inner.area {
                if outer.area == inner.area {
                    ambiguous[i] = true;
                    ambiguous[j] = true;
                }
                continue;
            }
            let projected = inner
                .points
                .iter()
                .map(|&p| outer.frame.project(p))
                .collect::<Result<Vec<_>, _>>()?;
            let epsilon = 64. * f64::EPSILON * outer.points.len() as f64;
            if projected
                .iter()
                .all(|&p| point_in_mesh_hole_polygon(p, &outer.projected, epsilon))
            {
                if parent[i].is_none_or(|old: usize| loops[old].area > outer.area) {
                    parent[i] = Some(j);
                }
            } else {
                // Conservatively leave nonnested coplanar loops with
                // overlapping bounds open, as the mesh cap planner does.
                ambiguous[i] = true;
                ambiguous[j] = true;
            }
        }
    }
    let face_points = source
        .faces()
        .iter()
        .map(|face| {
            face.iter()
                .map(|&i| source.control_points()[i as usize])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let face_tree = RTree::bulk_load(
        face_points
            .iter()
            .enumerate()
            .map(|(index, p)| {
                Ok(BoxEntry {
                    index,
                    bounds: envelope(BoundingBox3::from_points(p.iter().copied())?),
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
    );
    let mut draft = Draft {
        points: source.control_points().to_vec(),
        faces: Vec::new(),
        boundary_edges: Vec::new(),
        boundaries: 0,
    };
    let mut children = vec![Vec::new(); loops.len()];
    for (i, &p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[p].push(i);
        }
    }
    let mut depths: Vec<Option<usize>> = vec![None; loops.len()];
    let mut blocked = ambiguous.clone();
    for i in 0..loops.len() {
        let mut path = Vec::new();
        let mut next = Some(i);
        while let Some(j) = next {
            if depths[j].is_some() {
                break;
            }
            path.push(j);
            next = parent[j];
        }
        let first_depth = next.and_then(|j| depths[j]).map_or(0, |d| d + 1);
        for (depth, j) in (first_depth..).zip(path.into_iter().rev()) {
            depths[j] = Some(depth);
            blocked[j] |= parent[j].is_some_and(|p| blocked[p]);
        }
    }
    for (i, outer) in loops.iter().enumerate() {
        if !depths[i].unwrap().is_multiple_of(2) || blocked[i] {
            continue;
        }
        let holes = &children[i];
        if holes.iter().any(|&h| blocked[h]) {
            continue;
        }
        let mut occupied = false;
        for entry in face_tree.locate_in_envelope_intersecting(envelope(outer.bounds)) {
            charge(&mut work, face_points[entry.index].len())?;
            if on_plane(&face_points[entry.index], &outer.plane, tolerance)? {
                occupied = true;
                break;
            }
        }
        if occupied {
            continue;
        }
        let mut indices = outer.vertices.clone();
        let mut projected = outer.projected.clone();
        let mut ranges = std::iter::once(0..indices.len()).collect::<Vec<_>>();
        for &h in holes {
            let start = indices.len();
            indices.extend_from_slice(&loops[h].vertices);
            projected.extend(
                loops[h]
                    .points
                    .iter()
                    .map(|&p| outer.frame.project(p))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            ranges.push(start..indices.len());
        }
        let Some(cdt) = triangulate_projected_mesh_region(&projected, &ranges)? else {
            continue;
        };
        let sign = projected_polygon_doubled_area(&outer.projected).signum();
        let convex = (0..outer.projected.len()).all(|k| {
            let p = &outer.projected;
            sign * mesh_hole_cross(p[k], p[(k + 1) % p.len()], p[(k + 2) % p.len()]) >= 0.
        });
        let start = draft.faces.len();
        if holes.is_empty() && convex {
            if !triangles || outer.vertices.len() == 3 {
                draft.faces.push(outer.vertices.clone());
            } else {
                let center = outer.bounds.center()?;
                let index = draft.points.len() as u32;
                draft.points.push(center);
                for k in 0..outer.vertices.len() {
                    let (a, b) = (
                        outer.vertices[k],
                        outer.vertices[(k + 1) % outer.vertices.len()],
                    );
                    match TriangleMesh::try_new(
                        vec![center, draft.points[a as usize], draft.points[b as usize]],
                        vec![[0, 1, 2]],
                        Tolerance::MESH_VALIDATION,
                    ) {
                        Ok(_) => draft.faces.push(vec![index, a, b]),
                        Err(GeometryError::DegenerateTriangle { .. }) => {}
                        Err(e) => return Err(e),
                    }
                }
                if draft.faces.len() == start {
                    draft.points.pop();
                }
            }
        } else {
            draft.faces.extend(
                cdt.into_iter()
                    .map(|t| t.map(|j| indices[j as usize]).to_vec()),
            );
        }
        if draft.faces.len() > start {
            draft.boundary_edges.extend_from_slice(&outer.edges);
            for &h in holes {
                draft.boundary_edges.extend_from_slice(&loops[h].edges);
            }
            draft.boundaries += 1 + holes.len();
        }
    }
    Ok(draft)
}
