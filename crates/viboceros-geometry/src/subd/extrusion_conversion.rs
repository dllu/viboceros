//! Whole extrusion conversion uses its native profile surface and solid caps.
use super::*;
use crate::{ExtrusionRepresentation, SubDFromSurfaceMethod, Tolerance};
use std::collections::{BTreeMap, BTreeSet};

impl SubD {
    /// Converts the public complete-profile NURBS form. As in the captured
    /// native behavior, caps are added only when both ends are capped and the
    /// complete profile is closed. Multiple profiles retain its joining policy.
    pub fn try_from_extrusion(
        extrusion: &ExtrusionRepresentation,
        method: SubDFromSurfaceMethod,
        corners: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let form = extrusion.nurbs_form()?;
        let (converted, mesh) = super::surface_conversion::convert_with_mesh(
            &form.surface,
            method,
            corners,
            tolerance,
        )?;
        let closed = [form.surface.is_closed_u()?, form.surface.is_closed_v()?];
        if !form.caps.into_iter().all(|c| c) || !closed[1 - form.path_axis] {
            return Ok(converted);
        }
        add_caps(&converted, &mesh, form.path_axis)
    }
}
fn key(p: Point3) -> [u64; 3] {
    p.to_array().map(|x| if x == 0. { 0 } else { x.to_bits() })
}
fn ordered_edge(a: u32, b: u32) -> [u32; 2] {
    [a.min(b), a.max(b)]
}
fn add_caps(subd: &SubD, mesh: &TriangleMesh, path_axis: usize) -> Result<SubD, GeometryError> {
    let mut incidences = BTreeMap::<[u32; 2], (usize, [u32; 2])>::new();
    for face in subd.faces() {
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let entry = incidences.entry(ordered_edge(a, b)).or_insert((0, [a, b]));
            entry.0 += 1;
        }
    }
    let mut next = BTreeMap::new();
    for (_, (count, [a, b])) in incidences {
        if count == 1 && next.insert(a, b).is_some() {
            return Err(GeometryError::SubDInterpolationFailed);
        }
    }
    // Use coordinates from the mesh that actually became the SubD. Rebuilding
    // rational surface controls can round their Euclidean locations differently.
    // Native control-net UVs label the path ends exactly, even after transposition.
    let uv = mesh
        .texture_coordinates()
        .ok_or(GeometryError::SubDInterpolationFailed)?;
    let ends: [BTreeSet<_>; 2] = std::array::from_fn(|end| {
        mesh.vertices()
            .iter()
            .zip(uv)
            .filter_map(|(&point, uv)| (uv[path_axis] == end as f64).then_some(key(point)))
            .collect()
    });
    let mut faces = subd.faces().to_vec();
    let mut capped = [false; 2];
    while let Some((&start, _)) = next.first_key_value() {
        let mut face = Vec::new();
        let mut vertex = start;
        loop {
            face.push(vertex);
            vertex = next
                .remove(&vertex)
                .ok_or(GeometryError::SubDInterpolationFailed)?;
            if vertex == start {
                break;
            }
            if face.len() > subd.control_points().len() {
                return Err(GeometryError::SubDInterpolationFailed);
            }
        }
        let end = (0..2)
            .find(|&e| {
                face.iter()
                    .all(|&v| ends[e].contains(&key(subd.control_points()[v as usize])))
            })
            .ok_or(GeometryError::SubDInterpolationFailed)?;
        if capped[end] || face.len() < 3 {
            return Err(GeometryError::SubDInterpolationFailed);
        }
        capped[end] = true;
        face.reverse();
        faces.push(face);
    }
    if capped != [true, true] {
        return Err(GeometryError::SubDInterpolationFailed);
    }
    let result = SubD::try_from_polygons(subd.control_points().to_vec(), faces)?;
    let creases = subd
        .edges()
        .iter()
        .zip(subd.edge_tags())
        .filter_map(|(&[a, b], &tag)| (tag == 2).then_some(ordered_edge(a, b)))
        .collect::<BTreeSet<_>>();
    let tags = result
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(i, &[a, b])| creases.contains(&ordered_edge(a, b)).then_some((i, 2)))
        .collect::<Vec<_>>();
    result.try_with_edge_tags(&tags)
}
