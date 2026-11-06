//! Closed-loop parity uses the stored seam; hole admission uses the full loop.
use super::*;

pub(super) fn groups(
    boundaries: &[PlanarSurfaceBoundary],
    tolerance: Tolerance,
) -> Result<Vec<Vec<usize>>, CommandError> {
    let mut unique = Vec::new();
    for (i, boundary) in boundaries.iter().enumerate() {
        let normalized = boundary.curve.try_reparameterized(0. ..=1.)?;
        let mut duplicate = false;
        for (_, curve) in &unique {
            if super::equivalent::curves(curve, &normalized, tolerance)? {
                duplicate = true;
                break;
            }
        }
        if !duplicate {
            unique.push((i, normalized));
        }
    }
    let count = unique.len();
    let mut contains = vec![vec![false; count]; count];
    for (i, (outer, _)) in unique.iter().enumerate() {
        for (j, (inner, _)) in unique.iter().enumerate() {
            if i != j {
                contains[i][j] =
                    contains_start(&boundaries[*outer], &boundaries[*inner], tolerance)?;
            }
        }
    }
    let depths = (0..count)
        .map(|i| contains.iter().filter(|r| r[i]).count())
        .collect::<Vec<_>>();
    let mut parents = vec![None; count];
    for child in 0..count {
        if depths[child] % 2 == 0 {
            continue;
        }
        let mut candidates = (0..count)
            .filter(|&i| contains[i][child] && depths[i] % 2 == 0 && depths[i] < depths[child])
            .collect::<Vec<_>>();
        candidates.sort_by(|&i, &j| {
            depths[j]
                .cmp(&depths[i])
                .then_with(|| {
                    boundaries[unique[i].0]
                        .area
                        .total_cmp(&boundaries[unique[j].0].area)
                })
                .then(i.cmp(&j))
        });
        for parent in candidates {
            let a = &boundaries[unique[parent].0];
            let b = &boundaries[unique[child].0];
            if a.area > b.area
                && !contact(&a.curve, &b.curve, tolerance)?
                && Brep::try_planar_face_with_holes(
                    &a.curve,
                    std::slice::from_ref(&b.curve),
                    tolerance,
                )
                .is_ok()
            {
                parents[child] = Some(parent);
                break;
            }
        }
    }
    Ok((0..count)
        .filter(|&i| parents[i].is_none())
        .map(|i| {
            std::iter::once(unique[i].0)
                .chain(
                    (0..count)
                        .filter(|&j| parents[j] == Some(i))
                        .map(|j| unique[j].0),
                )
                .collect()
        })
        .collect())
}
fn contains_start(
    outer: &PlanarSurfaceBoundary,
    inner: &PlanarSurfaceBoundary,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    if !bounds_contain_point(outer.bounds, inner.probe, tolerance) {
        return Ok(false);
    }
    for (plane, curve) in [(outer.plane, &inner.curve), (inner.plane, &outer.curve)] {
        for p in curve.control_points() {
            if plane
                .origin()
                .vector_to(p.point())?
                .dot(plane.z_axis().as_vector())?
                .abs()
                > tolerance.absolute()
            {
                return Ok(false);
            }
        }
    }
    let c = &outer.curve;
    let t = c.closest_parameter(inner.probe, tolerance)?;
    if c.evaluate(t)?.distance_to(inner.probe)? <= tolerance.absolute() {
        return Ok(false);
    }
    let face = &outer.untrimmed_face.faces()[0];
    let (u, v) = face.surface().closest_parameters(inner.probe, tolerance)?;
    if face.surface().evaluate(u, v)?.distance_to(inner.probe)? > tolerance.absolute() {
        return Ok(false);
    }
    Ok(face.contains_parameters(u, v, tolerance)?)
}

// A positive witness avoids subdivision around attained tangent contacts. A
// closest-point proposal is accepted only after checking its original spatial
// image. Failure to find a witness delegates to the general event query.
fn contact(a: &NurbsCurve, b: &NurbsCurve, tolerance: Tolerance) -> Result<bool, CommandError> {
    for (curve, other) in [(a, b), (b, a)] {
        let bounds = other.control_point_bounds();
        for t in curve
            .knots()
            .chunk_by(|a, b| a == b)
            .map(|group| group[0])
            .filter(|t| curve.domain().contains(t))
            .take(16)
        {
            let p = curve.evaluate(t)?;
            if !bounds_contain_point(bounds, p, tolerance) {
                continue;
            }
            let s = other.closest_parameter(p, tolerance)?;
            if p.distance_to(other.evaluate(s)?)? <= tolerance.absolute() {
                return Ok(true);
            }
        }
    }
    curves_contact(a, b, tolerance)
}
