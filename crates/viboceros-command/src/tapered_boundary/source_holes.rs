use super::*;
use viboceros_geometry::{BrepLoop, BrepLoopType, Curve3, CurveJoinOptions, CurveJoinStyle};

/// A contracting opening can disappear during the full temporary sweep.
/// Only fill it when its control hull proves that the offset cannot fit.
pub(super) fn fill_collapsed(
    source: &Brep,
    offset: f64,
    tolerance: Tolerance,
) -> Result<Option<(Brep, Vec<usize>)>, CommandError> {
    let face = &source.faces()[0];
    if face.loops().len() < 2 {
        return Ok(None);
    }
    let Some(plane) = face.surface().plane(tolerance)? else {
        return Ok(None);
    };
    let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let mut collapsed = Vec::new();
    for (index, boundary) in face.loops().iter().enumerate() {
        if boundary.loop_type() == BrepLoopType::Inner
            && half_width(source, boundary, frame)?
                .is_some_and(|radius| offset > radius + tolerance.absolute())
        {
            collapsed.push((0, index));
        }
    }
    if collapsed.is_empty() {
        return Ok(None);
    }
    let filled = source
        .try_remove_holes(&collapsed, tolerance)?
        .ok_or(REJECTED)?;
    Ok(Some((
        filled,
        collapsed.into_iter().map(|(_, index)| index).collect(),
    )))
}

fn half_width(
    source: &Brep,
    boundary: &BrepLoop,
    frame: Frame3,
) -> Result<Option<f64>, GeometryError> {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for trim in boundary.trims() {
        let Some(edge) = trim.edge() else {
            return Ok(None);
        };
        for control in source.edges()[edge].curve().control_points() {
            if control.weight() <= 0. {
                return Ok(None);
            }
            let coordinates = frame.projected_coordinates_of(control.point())?;
            for axis in 0..2 {
                min[axis] = min[axis].min(coordinates[axis]);
                max[axis] = max[axis].max(coordinates[axis]);
            }
        }
    }
    let width = (max[0] - min[0]).min(max[1] - min[1]);
    Ok((width.is_finite() && width > 0.).then_some(width * 0.5))
}

/// Keep the original base trim and sew a separate disk over each vanished hole.
pub(super) fn restore_base(
    result: Brep,
    source: &Brep,
    holes: &[usize],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let plane = source.faces()[0]
        .surface()
        .plane(tolerance)?
        .ok_or(REJECTED)?;
    let walls = result
        .faces()
        .iter()
        .enumerate()
        .filter_map(|(index, face)| (!on_plane(face.surface(), plane, tolerance)).then_some(index))
        .collect::<Vec<_>>();
    if walls.len() + 1 != result.faces().len() {
        return Err(REJECTED);
    }
    let walls = result.sub_brep(&walls, tolerance)?;
    let disks = holes
        .iter()
        .map(|&hole| hole_disk(source, hole, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let parts = [&walls, source]
        .into_iter()
        .chain(&disks)
        .collect::<Vec<_>>();
    let mut joined = viboceros_geometry::join_breps(&parts, tolerance.absolute(), tolerance)?;
    if joined.len() != 1 || joined[0].brep.is_solid() != result.is_solid() {
        return Err(REJECTED);
    }
    Ok(joined.remove(0).brep)
}

fn hole_disk(source: &Brep, hole: usize, tolerance: Tolerance) -> Result<Brep, CommandError> {
    let mut loops = source.loop_boundary_curve_components(0, hole)?;
    if loops.len() != 1 {
        return Err(REJECTED);
    }
    let chain = loops.remove(0);
    let boundary = if let [curve] = chain.as_slice() {
        curve.clone()
    } else {
        let curves = chain
            .into_iter()
            .map(Curve3::NurbsCurve)
            .collect::<Vec<_>>();
        let joined = viboceros_geometry::join_curves(
            &curves,
            CurveJoinOptions {
                tolerance: tolerance.absolute(),
                preserve_direction: true,
                style: CurveJoinStyle::Batch,
            },
            tolerance,
        )?;
        let [joined] = joined.as_slice() else {
            return Err(REJECTED);
        };
        joined.curve().as_ref().to_nurbs()?
    };
    Ok(Brep::try_planar_face(&boundary, tolerance)?)
}
