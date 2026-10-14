//! Native contact solving in a radius-scaled frame and normalized parameter chart.
use super::working_curve::normalized;
use super::*;
use viboceros_geometry::{
    AffineTransform3, CircularArc3, CurveFilletContact, CurveSegment3, Vector3,
};

unsafe extern "C" {
    fn vb_curve_fillet_contacts(
        first: *const NativeCurve,
        second: *const NativeCurve,
        radius: f64,
        tolerance: f64,
        parameters: *mut f64,
        arc_samples: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// `None` retains the portable producer for bases outside canonical import.
pub(super) fn contacts(
    first: &CurveSegment3,
    second: &CurveSegment3,
    radius: f64,
    tolerance: Tolerance,
) -> Result<Option<CurveFilletContact>, Error> {
    let origin = first.as_ref().end_point()?;
    let Some(a) = normalized(&first.to_nurbs()?, origin, radius)? else {
        return Ok(None);
    };
    let Some(b) = normalized(&second.to_nurbs()?, origin, radius)? else {
        return Ok(None);
    };
    let a = KernelCurve::from_nurbs(&a)?;
    let b = KernelCurve::from_nurbs(&b)?;
    let solver_tolerance = (tolerance.absolute() / radius * 0.001)
        .min(tolerance.angular() * 0.001)
        .max(64. * f64::EPSILON);
    let mut parameters = [0.; 2];
    let mut samples = [0.; 9];
    call(|error, capacity| unsafe {
        vb_curve_fillet_contacts(
            a.handle.as_ptr(),
            b.handle.as_ptr(),
            1.,
            solver_tolerance,
            parameters.as_mut_ptr(),
            samples.as_mut_ptr(),
            error,
            capacity,
        )
    })?;
    if parameters
        .iter()
        .any(|p| !p.is_finite() || *p <= 0. || *p >= 1.)
    {
        return Err(Error::InvalidOutput);
    }
    let arc = CircularArc3::try_from_three_points(
        Point3::try_new(samples[0], samples[1], samples[2])?,
        Point3::try_new(samples[3], samples[4], samples[5])?,
        Point3::try_new(samples[6], samples[7], samples[8])?,
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let restore = AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.)?, radius)?.then(
        AffineTransform3::from_translation(Vector3::try_from(origin.to_array())?),
    )?;
    let arc = arc
        .transformed_similarity(restore, Tolerance::NUMERICAL_VALIDATION)?
        .ok_or(Error::InvalidOutput)?;
    // A circular arc's analytic angle parameter differs from its exact
    // rational-quadratic representation. Recover that parameter from the
    // contact point, while native NURBS/line charts map affinely.
    let first_parameter = if matches!(first, CurveSegment3::Arc(_)) {
        first.as_ref().closest_parameter(arc.start()?, tolerance)?
    } else {
        first.parameter_at(parameters[0])?
    };
    let second_parameter = if matches!(second, CurveSegment3::Arc(_)) {
        second.as_ref().closest_parameter(arc.end()?, tolerance)?
    } else {
        second.parameter_at(parameters[1])?
    };
    Ok(Some(CurveFilletContact {
        parameters: [first_parameter, second_parameter],
        arc,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn far_origin_small_corner_contacts_are_accurate_but_retained_tangency_loses_precision() {
        let p = |x, y| Point3::try_new(100. + x * 1e-6, -100. + y * 1e-6, 0.).unwrap();
        let first = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2., 0.), p(2., 2.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let second = CurveSegment3::Line(
            viboceros_geometry::LineSegment::try_new(p(2., 2.), p(6., 2.), Tolerance::DEFAULT)
                .unwrap(),
        );
        let contact = contacts(
            &CurveSegment3::NurbsCurve(first.clone()),
            &second,
            0.5e-6,
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
        let original = first
            .evaluate_with_derivative(contact.parameters[0])
            .unwrap();
        let retained =
            super::super::trim_curve(&first, *first.domain().start()..=contact.parameters[0])
                .unwrap();
        let actual = retained
            .evaluate_with_derivative(*retained.domain().end())
            .unwrap();
        let tangent = viboceros_geometry::CurveRef::Arc(&contact.arc)
            .evaluate_with_tangent(*contact.arc.domain().start())
            .unwrap()
            .tangent()
            .as_vector();
        let original_direction = original.1.normalized_nonzero().unwrap().as_vector();
        let retained_direction = actual.1.normalized_nonzero().unwrap().as_vector();
        assert!(
            original
                .0
                .distance_to(contact.arc.start().unwrap())
                .unwrap()
                < Tolerance::DEFAULT.absolute()
        );
        assert!(
            original_direction.cross(tangent).unwrap().length().unwrap()
                < Tolerance::DEFAULT.angular()
        );
        assert!(
            retained_direction.cross(tangent).unwrap().length().unwrap()
                > Tolerance::DEFAULT.angular()
        );
    }
}
