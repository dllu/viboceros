//! Shared planar-region planning and exact Rust face assembly.
use super::*;
mod equivalent;
mod independent;
mod regions;

struct PlanarSurfaceBoundary {
    id: ObjectId,
    curve: NurbsCurve,
    untrimmed_face: Brep,
    area: Real,
    bounds: BoundingBox3,
    probe: Point3,
    plane: Frame3,
}

fn bounds_overlap(left: BoundingBox3, right: BoundingBox3, tolerance: Tolerance) -> bool {
    left.min()
        .to_array()
        .into_iter()
        .zip(left.max().to_array())
        .zip(
            right
                .min()
                .to_array()
                .into_iter()
                .zip(right.max().to_array()),
        )
        .all(|((left_min, left_max), (right_min, right_max))| {
            (left_min <= right_max || tolerance.approx_eq(left_min, right_max))
                && (right_min <= left_max || tolerance.approx_eq(right_min, left_max))
        })
}

fn bounds_contain_point(bounds: BoundingBox3, point: Point3, tolerance: Tolerance) -> bool {
    bounds
        .min()
        .to_array()
        .into_iter()
        .zip(bounds.max().to_array())
        .zip(point.to_array())
        .all(|((min, max), coordinate)| {
            (min <= coordinate || tolerance.approx_eq(min, coordinate))
                && (coordinate <= max || tolerance.approx_eq(coordinate, max))
        })
}

pub(super) fn curves_contact(
    a: &NurbsCurve,
    b: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    if !bounds_overlap(
        a.control_point_bounds(),
        b.control_point_bounds(),
        tolerance,
    ) {
        return Ok(false);
    }
    Ok(!a.intersection_events_with_curve(b, tolerance)?.is_empty())
}

pub(super) fn stage(
    curves: Vec<(ObjectId, NurbsCurve)>,
    tolerance: Tolerance,
    independent_contacts: bool,
) -> Result<(Vec<Brep>, Vec<ObjectId>), CommandError> {
    stage_with(curves, tolerance, independent_contacts, construct)
}
pub(super) fn construct(
    profiles: &[(ObjectId, NurbsCurve)],
    outer: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<Brep>, CommandError> {
    if profiles.len() == 1 {
        return Ok(vec![outer.clone()]);
    }
    Ok(vec![Brep::try_planar_face_with_overlapping_holes(
        &profiles[0].1,
        &profiles[1..]
            .iter()
            .map(|(_, c)| c.clone())
            .collect::<Vec<_>>(),
        tolerance,
    )?])
}
pub(super) fn stage_with(
    curves: Vec<(ObjectId, NurbsCurve)>,
    tolerance: Tolerance,
    independent_contacts: bool,
    mut produce: impl FnMut(
        &[(ObjectId, NurbsCurve)],
        &Brep,
        Tolerance,
    ) -> Result<Vec<Brep>, CommandError>,
) -> Result<(Vec<Brep>, Vec<ObjectId>), CommandError> {
    let mut boundaries = Vec::new();
    for (id, curve) in curves {
        if curve.is_closed()? && curve.is_planar(tolerance)? {
            if boundaries.len() == 128 {
                return Err(CommandError::Usage(
                    "PlanarSrf supports at most 128 closed planar boundaries",
                ));
            }
            let untrimmed_face = Brep::try_planar_face(&curve, tolerance)?;
            let plane = viboceros_geometry::CurveRef::NurbsCurve(&curve)
                .supporting_plane_frame(tolerance)?
                .ok_or(GeometryError::InvalidPlanarFaceBoundary)?;
            let area = untrimmed_face.area(tolerance)?;
            let bounds = curve.control_point_bounds();
            let probe = curve.evaluate(*curve.domain().start())?;
            boundaries.push(PlanarSurfaceBoundary {
                id,
                curve,
                untrimmed_face,
                area,
                bounds,
                probe,
                plane,
            });
        }
    }
    if boundaries.is_empty() {
        return Err(CommandError::NoPlanarSurfaceBoundaries);
    }

    let groups = if independent_contacts {
        independent::groups(&boundaries, tolerance)?
    } else {
        regions::groups(&boundaries, tolerance)?
    };
    let mut faces = Vec::with_capacity(groups.len());
    for group in groups {
        let profiles = group
            .iter()
            .map(|&i| (boundaries[i].id, boundaries[i].curve.clone()))
            .collect::<Vec<_>>();
        faces.extend(produce(
            &profiles,
            &boundaries[group[0]].untrimmed_face,
            tolerance,
        )?);
    }
    Ok((faces, boundaries.iter().map(|b| b.id).collect()))
}
