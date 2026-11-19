//! Identify the sweep and cutter faces that bound the retained section.
use super::*;
use crate::extrude_surface::sweep_surface_key;
use std::collections::HashMap;
use std::ops::RangeInclusive;

pub(super) fn select_section(
    result: Brep,
    original: &Brep,
    source: &Brep,
    end: &NurbsSurface,
    canonical_tool: &Brep,
    boundary: &Brep,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let carrier = boundary.faces()[0].surface();
    let base = original
        .faces()
        .iter()
        .find(|face| copied_carrier(face.surface(), source.faces()[0].surface(), tolerance))
        .ok_or(REJECTED)?
        .surface();
    let end = original
        .faces()
        .iter()
        .find(|face| copied_carrier(face.surface(), end, tolerance))
        .ok_or(REJECTED)?
        .surface();
    let cap = canonical_tool
        .faces()
        .iter()
        .find(|face| copied_carrier(face.surface(), carrier, tolerance))
        .ok_or(REJECTED)?
        .surface();
    let mut walls = HashMap::<u64, Vec<&NurbsSurface>>::new();
    for face in original.faces() {
        let surface = face.surface();
        if surface != base && surface != end {
            walls
                .entry(sweep_surface_key(surface))
                .or_default()
                .push(surface);
        }
    }

    let plane = base.plane(tolerance)?;
    let cap_plane = cap.plane(tolerance)?;
    let on_carrier = |surface: &NurbsSurface, carrier, plane: Option<Plane>| {
        surface == carrier || plane.is_some_and(|plane| on_plane(surface, plane, tolerance))
    };
    let mut roles = result
        .faces()
        .iter()
        .map(|face| {
            let surface = face.surface();
            if on_carrier(surface, base, plane) {
                CutFace::Base
            } else if on_carrier(surface, cap, cap_plane) {
                CutFace::Cap
            } else if walls
                .get(&sweep_surface_key(surface))
                .is_some_and(|matches| matches.contains(&surface))
            {
                CutFace::Wall
            } else {
                CutFace::Other
            }
        })
        .collect::<Vec<_>>();
    let has_holes = boundary.faces()[0].loops().len() > 1;
    let result = if has_holes {
        // Removing the tool's side walls and the far cap disconnects material
        // that survives above an enclosed cutter hole.
        let kept = roles
            .iter()
            .enumerate()
            .filter_map(|(index, role)| (*role != CutFace::Other).then_some(index))
            .collect::<Vec<_>>();
        if kept.is_empty() {
            return Err(REJECTED);
        }
        roles.retain(|role| *role != CutFace::Other);
        result.sub_brep(&kept, tolerance)?
    } else {
        result
    };
    let mut kept = Vec::new();
    let mut base_faces = Vec::new();
    let mut cap_found = false;
    for component in result.edge_connected_face_components() {
        if !component.iter().any(|&i| roles[i] == CutFace::Base) {
            continue;
        }
        for index in component {
            match roles[index] {
                CutFace::Base => base_faces.push(index),
                CutFace::Cap => cap_found = true,
                CutFace::Wall => {}
                CutFace::Other => return Err(REJECTED),
            }
            kept.push(index);
        }
    }
    if !cap_found || base_faces.is_empty() {
        return Err(REJECTED);
    }
    let area = source.area(tolerance)?;
    let retained_area = result.sub_brep(&base_faces, tolerance)?.area(tolerance)?;
    let source_bounds = source.bounds();
    let diagonal = source_bounds.min().distance_to(source_bounds.max())?;
    if (retained_area - area).abs()
        > 4. * diagonal * tolerance.absolute() + area * tolerance.relative()
    {
        return Err(REJECTED);
    }
    let result = if kept.len() == result.faces().len() {
        result
    } else {
        result.sub_brep(&kept, tolerance)?
    };
    if !result.is_solid()
        && !(has_holes
            && only_cap_holes_are_open(&result, |surface| on_carrier(surface, cap, cap_plane)))
    {
        return Err(REJECTED);
    }
    Ok(result)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CutFace {
    Base,
    Cap,
    Wall,
    Other,
}

fn only_cap_holes_are_open(brep: &Brep, is_cap: impl Fn(&NurbsSurface) -> bool) -> bool {
    use viboceros_geometry::BrepLoopType;

    let mut edges = vec![[0u8; 2]; brep.edges().len()];
    let mut hole_edges = vec![false; edges.len()];
    for face in brep.faces() {
        for boundary in face.loops() {
            let cap_hole = boundary.loop_type() == BrepLoopType::Inner && is_cap(face.surface());
            for trim in boundary.trims() {
                if let Some(edge) = trim.edge() {
                    let count =
                        &mut edges[edge][usize::from(trim.is_reversed_3d() ^ face.is_reversed())];
                    *count = count.saturating_add(1);
                    hole_edges[edge] |= cap_hole;
                }
            }
        }
    }
    edges
        .into_iter()
        .zip(hole_edges)
        .all(|(uses, hole)| uses == [1, 1] || hole && matches!(uses, [0, 1] | [1, 0]))
}

fn copied_carrier(copy: &NurbsSurface, original: &NurbsSurface, tolerance: Tolerance) -> bool {
    if copy == original {
        return true;
    }
    if let Ok(Some(plane)) = original.plane(tolerance) {
        return on_plane(copy, plane, tolerance);
    }
    // Import can change the orientation and affine domain of a curved cap.
    let a = axes(original);
    let b = axes(copy);
    let relative = tolerance.relative() + 32. * f64::EPSILON;
    let close = |x: f64, y: f64| (x - y).abs() <= relative * x.abs().max(y.abs()).max(1.);
    for order in [[0, 1], [1, 0]] {
        if (0..2).any(|axis| (a[axis].0, a[axis].1) != (b[order[axis]].0, b[order[axis]].1)) {
            continue;
        }
        for reversed in [[false, false], [true, false], [false, true], [true, true]] {
            if (0..2).any(|axis| {
                let (_, _, knots, domain) = &a[axis];
                let (_, _, other, other_domain) = &b[order[axis]];
                let normalized = |knot, interval: &RangeInclusive<f64>| {
                    (knot - interval.start()) / (interval.end() - interval.start())
                };
                knots.iter().enumerate().any(|(i, &knot)| {
                    let next = if reversed[axis] {
                        1. - normalized(other[other.len() - 1 - i], other_domain)
                    } else {
                        normalized(other[i], other_domain)
                    };
                    !close(normalized(knot, domain), next)
                })
            }) {
                continue;
            }
            let control = |u, v| {
                let mut at = [0; 2];
                for (axis, index) in [u, v].into_iter().enumerate() {
                    at[order[axis]] = if reversed[axis] {
                        a[axis].1 - 1 - index
                    } else {
                        index
                    };
                }
                copy.control_point(at[0], at[1]).unwrap()
            };
            let original_weight = original.control_point(0, 0).unwrap().weight();
            let copied_weight = control(0, 0).weight();
            if (0..a[1].1).all(|v| {
                (0..a[0].1).all(|u| {
                    let expected = original.control_point(u, v).unwrap();
                    let actual = control(u, v);
                    actual.point().is_near(expected.point(), tolerance)
                        && close(
                            actual.weight() / copied_weight,
                            expected.weight() / original_weight,
                        )
                })
            }) {
                return true;
            }
        }
    }
    false
}

type SurfaceAxis<'a> = (usize, usize, &'a [f64], RangeInclusive<f64>);

fn axes(surface: &NurbsSurface) -> [SurfaceAxis<'_>; 2] {
    [
        (
            surface.degree_u(),
            surface.control_point_count_u(),
            surface.knots_u(),
            surface.domain_u(),
        ),
        (
            surface.degree_v(),
            surface.control_point_count_v(),
            surface.knots_v(),
            surface.domain_v(),
        ),
    ]
}

pub(super) fn on_plane(surface: &NurbsSurface, plane: Plane, tolerance: Tolerance) -> bool {
    surface.control_points().iter().all(|control| {
        plane
            .signed_distance_to(control.point())
            .is_ok_and(|distance| distance.abs() <= tolerance.absolute())
    })
}
