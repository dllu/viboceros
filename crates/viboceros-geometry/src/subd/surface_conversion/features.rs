//! Public G1 queries and control-net crease transfer.
use super::*;
use std::collections::{HashMap, HashSet};

pub(super) fn discontinuities(
    surface: &NurbsSurface,
    regular_only: [bool; 2],
) -> Result<[Vec<f64>; 2], GeometryError> {
    let controls = surface
        .control_points()
        .iter()
        .map(|p| {
            let [x, y, z] = p.point().to_array();
            [x, y, z, p.weight()]
        })
        .collect::<Vec<_>>();
    Ok(viboceros_opennurbs::surface_discontinuities(
        viboceros_opennurbs::SurfaceDefinition {
            degree: [surface.degree_u() as u32, surface.degree_v() as u32],
            count: [
                surface.control_point_count_u(),
                surface.control_point_count_v(),
            ],
            controls: &controls,
            knots: [surface.knots_u(), surface.knots_v()],
        },
        regular_only,
    )?)
}

fn key(p: Point3) -> [u64; 3] {
    p.to_array().map(|x| if x == 0. { 0 } else { x.to_bits() })
}
pub(super) fn apply_creases(subd: SubD, pairs: &[[Point3; 2]]) -> Result<SubD, GeometryError> {
    if pairs.is_empty() {
        return Ok(subd);
    }
    let indices = subd
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, &p)| (key(p), i as u32))
        .collect::<HashMap<_, _>>();
    let mut wanted = HashSet::new();
    for &[a, b] in pairs {
        if a == b {
            continue;
        }
        let a = *indices
            .get(&key(a))
            .ok_or(GeometryError::SubDInterpolationFailed)?;
        let b = *indices
            .get(&key(b))
            .ok_or(GeometryError::SubDInterpolationFailed)?;
        wanted.insert([a.min(b), a.max(b)]);
    }
    let tags = subd
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(i, &[a, b])| wanted.remove(&[a.min(b), a.max(b)]).then_some((i, 2)))
        .collect::<Vec<_>>();
    if !wanted.is_empty() {
        return Err(GeometryError::SubDInterpolationFailed);
    }
    subd.try_with_edge_tags(&tags)
}

pub(super) fn crease_pairs(
    points: &[Point3],
    counts: [usize; 2],
    anchors: &[Vec<usize>; 2],
) -> Vec<[Point3; 2]> {
    let mut pairs = Vec::new();
    for axis in 0..2 {
        for &i in &anchors[axis] {
            for j in 1..counts[1 - axis] {
                let index = |k| {
                    if axis == 0 {
                        k * counts[0] + i
                    } else {
                        i * counts[0] + k
                    }
                };
                pairs.push([points[index(j - 1)], points[index(j)]]);
            }
        }
    }
    pairs
}
