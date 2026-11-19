//! Cut closed sweeps to a finite boundary face and keep the source section.
use super::*;
use crate::extrude_surface::{split_wall_edges, sweep_surface_key};
use std::collections::HashMap;
use std::ops::RangeInclusive;
use viboceros_smlib::{BooleanOperation, Shape, Solid};

const REJECTED: CommandError = CommandError::SurfaceExtrusionBoundaryRejected;

pub(super) struct BoundaryDifference {
    stock: Solid,
    tool: Solid,
    pub(super) result: Brep,
}

impl BoundaryDifference {
    pub(super) fn subtract(
        stock: &Brep,
        boundary: &Brep,
        offset: Vector3,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, CommandError> {
        let stock = Solid::from_brep(stock, tolerance)?;
        let tool = boundary_tool(boundary, offset, tolerance)?;
        let result = stock.boolean_with_postprocess(&tool, BooleanOperation::Difference, false)?;
        if result.is_empty()? {
            return Ok(None);
        }
        Ok(Some(Self {
            stock,
            tool,
            result: result.to_brep(tolerance)?,
        }))
    }

    pub(super) fn keep_source_section(
        self,
        source: &Brep,
        end: &NurbsSurface,
        boundary: &Brep,
        tolerance: Tolerance,
    ) -> Result<Brep, CommandError> {
        select_section(
            self.result,
            &self.stock.to_brep(tolerance)?,
            source,
            end,
            &self.tool.to_brep(tolerance)?,
            boundary,
            tolerance,
        )
    }
}

pub(super) fn boundary_tool(
    boundary: &Brep,
    offset: Vector3,
    tolerance: Tolerance,
) -> Result<Solid, CommandError> {
    let profile = split_wall_edges(boundary, false, tolerance)?;
    let tool = Shape::from_brep(&profile, tolerance)?
        .extruded_faces(offset, true, tolerance)?
        .to_brep(tolerance)?;
    Ok(Solid::from_brep(&tool, tolerance)?)
}

fn select_section(
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
    let mut wall_carriers = Vec::new();
    for face in original.faces() {
        let surface = face.surface();
        if surface != base && surface != end {
            walls
                .entry(sweep_surface_key(surface))
                .or_default()
                .push(surface);
            wall_carriers.push(surface);
        }
    }

    let plane = base.plane(tolerance)?;
    let cap_plane = cap.plane(tolerance)?;
    let on_carrier = |surface: &NurbsSurface, carrier, plane: Option<Plane>| {
        surface == carrier || plane.is_some_and(|plane| on_plane(surface, plane, tolerance))
    };
    let mut comparisons = 1_000_000usize;
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
                || matches_reparameterized_wall(
                    surface,
                    &wall_carriers,
                    &mut comparisons,
                    tolerance,
                )
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

pub(super) fn copied_carrier(
    copy: &NurbsSurface,
    original: &NurbsSurface,
    tolerance: Tolerance,
) -> bool {
    if copy == original {
        return true;
    }
    if let Ok(Some(plane)) = original.plane(tolerance) {
        return on_plane(copy, plane, tolerance);
    }
    same_spline_carrier(copy, original, tolerance)
}

/// Transfer a planar cap and its existing topology to the selected cutter's
/// chart. The cutter supplies exact p-curves for the cap's shared 3D edges.
pub(super) fn mount_planar_cap(
    result: Brep,
    cap: usize,
    cutter: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, SurfaceIso};

    let old = &result.faces()[cap];
    let normal = |surface: &NurbsSurface| {
        surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)
    };
    let flip = normal(cutter)?
        .as_vector()
        .dot(normal(old.surface())?.as_vector())?
        < 0.;
    let mut loops = Vec::with_capacity(old.loops().len());
    for boundary in old.loops() {
        let mut trims = Vec::with_capacity(boundary.trims().len());
        for trim in boundary.trims() {
            let edge = trim.edge().ok_or(REJECTED)?;
            let mut uv =
                cutter.try_pullback_exact_curve(result.edges()[edge].curve(), tolerance)?;
            let reversed = trim.is_reversed_3d() ^ flip;
            if reversed {
                uv = uv.reversed()?;
            }
            let [a, b] = trim.vertices();
            trims.push(BrepTrim::try_new(
                if flip { [b, a] } else { [a, b] },
                Some(edge),
                reversed,
                uv,
                trim.trim_type(),
                SurfaceIso::NotIso,
                [0.; 2],
            )?);
        }
        if flip {
            trims.reverse();
        }
        loops.push(BrepLoop::try_new(boundary.loop_type(), trims)?);
    }
    let mut faces = result.faces().to_vec();
    faces[cap] = BrepFace::try_new(cutter.clone(), old.is_reversed() ^ flip, loops)?;
    let remapped = Brep::try_new(
        result.vertices().to_vec(),
        result.edges().to_vec(),
        faces,
        tolerance,
    )?;
    if !remapped.is_solid() {
        return Err(REJECTED);
    }
    Ok(remapped)
}

fn matches_reparameterized_wall(
    surface: &NurbsSurface,
    originals: &[&NurbsSurface],
    comparisons: &mut usize,
    tolerance: Tolerance,
) -> bool {
    if surface.degree_u() == 1 && surface.degree_v() == 1 {
        return false;
    }
    for &wall in originals {
        let cost = surface
            .control_points()
            .len()
            .max(wall.control_points().len());
        let Some(remaining) = comparisons.checked_sub(cost) else {
            *comparisons = 0;
            return false;
        };
        *comparisons = remaining;
        if same_spline_carrier(surface, wall, tolerance) {
            return true;
        }
    }
    false
}

fn same_spline_carrier(copy: &NurbsSurface, original: &NurbsSurface, tolerance: Tolerance) -> bool {
    // The kernel can reorient a curved carrier or normalize its parameter domains.
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

pub(super) fn axial_point(point: Point3, direction: Vector3) -> Result<f64, GeometryError> {
    Vector3::try_from(point.to_array())?.dot(direction)
}

pub(super) fn axial_extent(
    bounds: BoundingBox3,
    direction: Vector3,
) -> Result<[f64; 2], GeometryError> {
    let [a, b] = [bounds.min().to_array(), bounds.max().to_array()];
    let axes = direction.to_array();
    let lo = Point3::try_from(std::array::from_fn(
        |i| if axes[i] < 0. { b[i] } else { a[i] },
    ))?;
    let hi = Point3::try_from(std::array::from_fn(
        |i| if axes[i] < 0. { a[i] } else { b[i] },
    ))?;
    Ok([axial_point(lo, direction)?, axial_point(hi, direction)?])
}

pub(super) fn hole_crosses_sweep(
    boundary: &Brep,
    sweep: &Brep,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    use viboceros_geometry::BrepLoopType;

    for boundary_loop in boundary.faces()[0].loops() {
        if boundary_loop.loop_type() != BrepLoopType::Inner {
            continue;
        }
        for edge in boundary_loop.trims().iter().filter_map(|trim| trim.edge()) {
            let curve = boundary.edges()[edge].curve();
            if !viboceros_smlib::curve_brep_intersection_events(curve, sweep, None, tolerance)?
                .is_empty()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
