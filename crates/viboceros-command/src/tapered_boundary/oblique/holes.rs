use super::*;
use std::collections::HashMap;
use viboceros_geometry::{BrepLoop, BrepLoopType, BrepTrim, BrepTrimType};

#[cfg(test)]
mod tests;

pub(super) enum SectionHoles {
    Enclosed(Vec<usize>),
    RimContact,
    CoversSection,
}

/// Compare cutter holes with the section on the shared cutter chart.
pub(super) fn section_holes(
    cutter: &Brep,
    closed: &Brep,
    section: &BrepFace,
    tolerance: Tolerance,
) -> Result<SectionHoles, CommandError> {
    let rims = section
        .loops()
        .iter()
        .flat_map(|rim| rim.trims())
        .filter_map(|trim| trim.edge())
        .map(|edge| closed.edges()[edge].curve())
        .collect::<Vec<_>>();
    let mut enclosed = Vec::new();
    for (index, hole) in cutter.faces()[0].loops().iter().enumerate().skip(1) {
        for trim in hole.trims() {
            let edge = cutter.edges()[trim.edge().ok_or(REJECTED)?].curve();
            for rim in &rims {
                if !viboceros_smlib::curve_intersection_events(edge, rim, tolerance)?.is_empty() {
                    return Ok(SectionHoles::RimContact);
                }
            }
        }
        // With disjoint boundaries, one point per loop establishes which
        // region contains it. Also check for a hole enveloping a section rim.
        let patch = BrepFace::try_new(
            section.surface().clone(),
            section.is_reversed(),
            vec![reversed_inner_loop(hole)?],
        )?;
        for rim in section.loops() {
            if contains_loop_start(&patch, rim, tolerance)? {
                return Ok(SectionHoles::CoversSection);
            }
        }
        if contains_loop_start(section, hole, tolerance)? {
            enclosed.push(index);
        }
    }
    Ok(SectionHoles::Enclosed(enclosed))
}

fn contains_loop_start(
    face: &BrepFace,
    boundary: &BrepLoop,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let curve = boundary.trims()[0].curve();
    let point = curve.evaluate(*curve.domain().start())?;
    face.contains_parameters(point.x(), point.y(), tolerance)
}

/// Add enclosed cutter holes on the shared cap chart. Planar cutters also
/// receive a separate patch for each hole.
pub(super) fn trim_cap(
    closed: Brep,
    cutter: &Brep,
    holes: &[usize],
    fill: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let mut vertices = closed.vertices().to_vec();
    let mut edges = closed.edges().to_vec();
    let mut faces = closed.faces().to_vec();
    let cap = faces.pop().ok_or(REJECTED)?;
    let mut cap_loops = cap.loops().to_vec();
    let mut vertex_ids = HashMap::new();
    let mut edge_ids = HashMap::new();
    let mut vertex_id = |index: usize| {
        *vertex_ids.entry(index).or_insert_with(|| {
            let id = vertices.len();
            vertices.push(cutter.vertices()[index]);
            id
        })
    };
    let mut patches = Vec::with_capacity(holes.len());
    let trim_type = if fill {
        BrepTrimType::Mated
    } else {
        BrepTrimType::Boundary
    };
    for &index in holes {
        let hole = &cutter.faces()[0].loops()[index];
        let mut trims = Vec::with_capacity(hole.trims().len());
        for trim in hole.trims() {
            let edge = *edge_ids
                .entry(trim.edge().ok_or(REJECTED)?)
                .or_insert_with_key(|&index| {
                    let edge = &cutter.edges()[index];
                    let id = edges.len();
                    edges.push(
                        edge.clone()
                            .with_vertices(edge.vertices().map(&mut vertex_id)),
                    );
                    id
                });
            trims.push(BrepTrim::try_new(
                trim.vertices().map(&mut vertex_id),
                Some(edge),
                trim.is_reversed_3d(),
                trim.curve().clone(),
                trim_type,
                trim.iso(),
                trim.tolerance(),
            )?);
        }
        let inner = BrepLoop::try_new(BrepLoopType::Inner, trims)?;
        if fill {
            patches.push(BrepFace::try_new(
                cap.surface().clone(),
                cap.is_reversed(),
                vec![reversed_inner_loop(&inner)?],
            )?);
        }
        cap_loops.push(inner);
    }
    faces.push(BrepFace::try_new(
        cap.surface().clone(),
        cap.is_reversed(),
        cap_loops,
    )?);
    faces.extend(patches);
    let result = Brep::try_new(vertices, edges, faces, tolerance)?;
    if result.is_solid() != fill {
        return Err(REJECTED);
    }
    Ok(result)
}

fn reversed_inner_loop(boundary: &BrepLoop) -> Result<BrepLoop, GeometryError> {
    let trims = boundary
        .trims()
        .iter()
        .rev()
        .map(|trim| {
            let [a, b] = trim.vertices();
            BrepTrim::try_new(
                [b, a],
                trim.edge(),
                trim.edge().is_some() && !trim.is_reversed_3d(),
                trim.curve().reversed()?,
                trim.trim_type(),
                trim.iso(),
                trim.tolerance(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    BrepLoop::try_new(BrepLoopType::Outer, trims)
}
