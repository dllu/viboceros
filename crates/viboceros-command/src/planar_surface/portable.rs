//! Portable planar-face assembly, excluded from native command builds.
use super::*;

struct PlanarSurfaceBoundary {
    id: ObjectId,
    curve: NurbsCurve,
    untrimmed_face: Brep,
    area: Real,
    bounds: BoundingBox3,
    probe: Point3,
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
    let mut boundaries = Vec::new();
    for (id, curve) in curves {
        if curve.is_closed()? && curve.is_planar(tolerance)? {
            let untrimmed_face = Brep::try_planar_face(&curve, tolerance)?;
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
            });
        }
    }
    if boundaries.is_empty() {
        return Err(CommandError::NoPlanarSurfaceBoundaries);
    }

    // A strict containment forest applies the same even-odd region rule
    // as planar CAD boundaries: children cut holes, grandchildren become
    // independent islands, and disjoint or partially overlapping loops
    // remain independent faces. The smallest containing loop is the
    // immediate parent when several levels are nested.
    let mut parents: Vec<Option<usize>> = vec![None; boundaries.len()];
    let mut indices_by_area = (0..boundaries.len()).collect::<Vec<_>>();
    indices_by_area.sort_by(|left, right| {
        boundaries[*left]
            .area
            .total_cmp(&boundaries[*right].area)
            .then(left.cmp(right))
    });
    for child_index in 0..boundaries.len() {
        for &outer_index in &indices_by_area {
            if outer_index == child_index
                || boundaries[outer_index].area <= boundaries[child_index].area
                || !bounds_contain_point(
                    boundaries[outer_index].bounds,
                    boundaries[child_index].probe,
                    tolerance,
                )
            {
                continue;
            }
            if Brep::try_planar_face_with_holes(
                &boundaries[outer_index].curve,
                std::slice::from_ref(&boundaries[child_index].curve),
                tolerance,
            )
            .is_ok()
                && (!independent_contacts
                    || !curves_contact(
                        &boundaries[outer_index].curve,
                        &boundaries[child_index].curve,
                        tolerance,
                    )?)
            {
                parents[child_index] = Some(outer_index);
                break;
            }
        }
    }

    // Intersecting siblings cannot both be trim holes. Rhino treats
    // partially overlapping input curves as independent surfaces, so
    // detach both while leaving unrelated siblings assigned as holes.
    let mut detached = vec![false; boundaries.len()];
    for parent_index in 0..boundaries.len() {
        let children = parents
            .iter()
            .enumerate()
            .filter_map(|(index, parent)| (*parent == Some(parent_index)).then_some(index))
            .collect::<Vec<_>>();
        for first_position in 0..children.len() {
            for second_position in (first_position + 1)..children.len() {
                let first = children[first_position];
                let second = children[second_position];
                if !bounds_overlap(
                    boundaries[first].bounds,
                    boundaries[second].bounds,
                    tolerance,
                ) {
                    continue;
                }
                let holes = [
                    boundaries[first].curve.clone(),
                    boundaries[second].curve.clone(),
                ];
                if Brep::try_planar_face_with_holes(
                    &boundaries[parent_index].curve,
                    &holes,
                    tolerance,
                )
                .is_err()
                {
                    detached[first] = true;
                    detached[second] = true;
                }
            }
        }
    }
    for (parent, detached) in parents.iter_mut().zip(detached) {
        if detached {
            *parent = None;
        }
    }
    let depths = (0..boundaries.len())
        .map(|index| {
            let mut depth = 0_usize;
            let mut ancestor = parents[index];
            while let Some(parent) = ancestor {
                depth += 1;
                debug_assert!(depth < boundaries.len());
                ancestor = parents[parent];
            }
            depth
        })
        .collect::<Vec<_>>();

    let mut faces = Vec::new();
    for outer_index in 0..boundaries.len() {
        if depths[outer_index] % 2 != 0 {
            continue;
        }
        let holes = parents
            .iter()
            .enumerate()
            .filter(|(_, parent)| **parent == Some(outer_index))
            .map(|(index, _)| boundaries[index].curve.clone())
            .collect::<Vec<_>>();
        let face = if holes.is_empty() {
            boundaries[outer_index].untrimmed_face.clone()
        } else {
            Brep::try_planar_face_with_holes(&boundaries[outer_index].curve, &holes, tolerance)?
        };
        faces.push(face);
    }

    Ok((faces, boundaries.iter().map(|b| b.id).collect()))
}
