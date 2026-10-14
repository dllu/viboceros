//! Continuous matched-parameter acceptance of an original trim boundary image.
use super::*;

fn directed_piece(curve: &NurbsCurve2, a: Real, b: Real) -> Result<NurbsCurve2, GeometryError> {
    let piece = curve.try_trimmed(a.min(b)..=a.max(b))?;
    if a > b { piece.reversed() } else { Ok(piece) }
}

pub(super) fn certified(
    surface: &NurbsSurface,
    boundary: &NurbsCurve2,
    parameters: [Real; 2],
    spatial: &NurbsCurve,
    limit: Real,
) -> Result<bool, GeometryError> {
    let [a, b] = parameters;
    if a == b {
        return Ok(false);
    }
    let domain = boundary.domain();
    crate::parameter::checked_parameter(a, domain.clone())?;
    crate::parameter::checked_parameter(b, domain.clone())?;
    let width = domain.end() - domain.start();
    require_finite([width, b - a], "surface trim boundary interval")?;
    if boundary.start_point()? != boundary.end_point()? || (b - a).abs() <= width * 0.5 {
        let uv = directed_piece(boundary, a, b)?;
        return Ok(surface
            .parameter_curve_deviation_bound(&uv, spatial, limit)?
            .is_some());
    }
    // A closed p-curve may cross its stored seam. The two original pieces
    // jointly cover the shorter directed arc; each gets a separate proof.
    let (first_end, second_start) = if a > b {
        (*domain.end(), *domain.start())
    } else {
        (*domain.start(), *domain.end())
    };
    let first_length = (first_end - a).abs();
    let second_length = (b - second_start).abs();
    if first_length == 0. && second_length == 0. {
        // Distinct stored seam endpoints can also describe the complete closed
        // trim, rather than a zero-length wrapped arc. Prove that full image.
        let uv = directed_piece(boundary, a, b)?;
        return Ok(surface
            .parameter_curve_deviation_bound(&uv, spatial, limit)?
            .is_some());
    }
    let fraction = first_length / (first_length + second_length);
    require_finite([fraction], "surface trim seam fraction")?;
    let middle = spatial.parameter_at(fraction)?;
    for (from, to, start, end) in [
        (a, first_end, *spatial.domain().start(), middle),
        (second_start, b, middle, *spatial.domain().end()),
    ] {
        if from == to {
            continue;
        }
        if start >= end {
            return Ok(false);
        }
        let uv = directed_piece(boundary, from, to)?;
        let piece = spatial.try_trimmed(start..=end)?;
        if surface
            .parameter_curve_deviation_bound(&uv, &piece, limit)?
            .is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    fn surface() -> NurbsSurface {
        NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]).unwrap()
    }
    fn square() -> NurbsCurve2 {
        NurbsCurve2::try_new(
            1,
            [[0., 0.], [1., 0.], [1., 1.], [0., 1.], [0., 0.]]
                .into_iter()
                .map(|p| Point2::try_from(p).unwrap())
                .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap()
    }
    #[test]
    fn closed_boundary_seam_crossings_are_certified_in_both_directions() {
        let spatial = NurbsCurve::try_new(
            1,
            vec![p(0., 0.25), p(0., 0.), p(0.25, 0.)],
            vec![10., 10., 12., 14., 14.],
        )
        .unwrap();
        assert!(certified(&surface(), &square(), [3.75, 0.25], &spatial, 1e-9).unwrap());
        assert!(
            certified(
                &surface(),
                &square(),
                [0.25, 3.75],
                &spatial.reversed().unwrap(),
                1e-9
            )
            .unwrap()
        );
    }
    #[test]
    fn complete_closed_boundary_images_do_not_form_an_empty_seam_arc() {
        let spatial = NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.), p(0., 0.)],
            vec![10., 10., 12., 14., 16., 18., 18.],
        )
        .unwrap();
        assert!(certified(&surface(), &square(), [0., 4.], &spatial, 1e-9).unwrap());
        assert!(
            certified(
                &surface(),
                &square(),
                [4., 0.],
                &spatial.reversed().unwrap(),
                1e-9
            )
            .unwrap()
        );
    }
    #[test]
    fn boundary_contact_endpoints_cannot_hide_a_bowed_interior() {
        let boundary = NurbsCurve2::try_line(
            Point2::try_new(0.25, 0.).unwrap(),
            Point2::try_new(0.75, 0.).unwrap(),
        )
        .unwrap();
        for (height, passed) in [(0., true), (0.2, false)] {
            let spatial = NurbsCurve::try_new(
                2,
                vec![p(0.25, 0.), p(0.5, height), p(0.75, 0.)],
                vec![-4., -4., -4., 7., 7., 7.],
            )
            .unwrap();
            assert_eq!(
                certified(&surface(), &boundary, [0., 1.], &spatial, 1e-9).unwrap(),
                passed
            );
        }
    }
    #[test]
    fn invalid_boundary_parameters_are_rejected() {
        let spatial =
            NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 0.)], vec![0., 0., 1., 1.]).unwrap();
        assert!(certified(&surface(), &square(), [-1., 1.], &spatial, 1e-9).is_err());
        assert!(!certified(&surface(), &square(), [0., 0.], &spatial, 1e-9).unwrap());
    }
}
