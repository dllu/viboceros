//! Physical aspect estimates for sparse conforming trim triangulation.
use super::*;

pub(super) fn triangulate(
    parameters: &mut Vec<Point2>,
    lengths: &[usize],
    surface: &NurbsSurface,
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    let boundary = &parameters[..lengths[0]];
    // The two isocurves can leave the trimmed material. A pole on the
    // unused support or an ill-conditioned metric must not block meshing.
    if let Ok(aspect) = aspect(surface, boundary)
        && aspect != [1.; 2]
        && let Ok(Some(triangles)) =
            triangulate_trim_region_scaled(parameters, lengths, &[], aspect)
    {
        return Ok(Some(triangles));
    }
    trim_region::triangulate(parameters, lengths, &[])
}

fn aspect(surface: &NurbsSurface, boundary: &[Point2]) -> Result<[Real; 2], GeometryError> {
    let span = sampling::parameter_bounds(boundary);
    let middle = span.map(|[lo, hi]| lo.midpoint(hi));
    let mut lengths = [0.; 2];
    for (axis, length) in lengths.iter_mut().enumerate() {
        let mut uv = middle;
        uv[axis] = span[axis][0];
        let mut previous = surface.evaluate(uv[0], uv[1])?;
        // Several chords also estimate the length of a closed isocurve,
        // whose two endpoint positions may be identical.
        for station in 1..=8 {
            uv[axis] = normalized_span_parameter(span[axis], station as Real / 8.)?;
            let point = surface.evaluate(uv[0], uv[1])?;
            *length += previous.distance_to(point)?;
            previous = point;
        }
    }
    let short = usize::from(lengths[1] < lengths[0]);
    let ratio = lengths[1 - short] / lengths[short];
    let mut aspect = [1.; 2];
    if ratio.is_finite() && ratio >= 4. {
        // A power of two also preserves collinearity in the UV predicates.
        // Limit the correction to keep both axes in their numerical range.
        aspect[short] = ((1u32 << (ratio.min(64.).log2() as u32)) as Real).recip();
    }
    Ok(aspect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_cylinder_strip_does_not_bridge_multiple_angular_grid_cells() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 53., 0., 3.).unwrap();
        let u = *surface.domain_u().end();
        let p = |x: Real, y| Point2::try_new(x * u, y).unwrap();
        let mut uv = (0..=24).map(|i| p(i as Real / 24., 0.)).collect::<Vec<_>>();
        uv.push(p(1., 1.125));
        uv.extend((0..=24).rev().map(|i| p(i as Real / 24., 3.)));
        uv.push(p(0., 1.125));
        let outer = uv.len();
        for y in [0.75, 1.5, 2.25] {
            uv.extend((1..24).map(|i| p(i as Real / 24., y)));
        }
        let legacy = triangulate_trim_region(&uv, &[outer]).unwrap().unwrap();
        let mesh = triangulate(&mut uv, &[outer], &surface).unwrap().unwrap();
        let longest = |faces: &[[u32; 3]]| {
            faces
                .iter()
                .map(|tri| {
                    let [a, b, c] = tri.map(|i| uv[i as usize].x());
                    a.max(b).max(c) - a.min(b).min(c)
                })
                .fold(0., Real::max)
        };
        assert!(longest(&legacy) > u / 12.);
        assert!(longest(&mesh) < u / 23.);
        assert_eq!(mesh.len(), legacy.len());
    }
}
