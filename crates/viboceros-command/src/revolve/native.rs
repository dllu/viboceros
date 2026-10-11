//! Existing native revolution construction and shared profile preparation.
use super::*;
use viboceros_smlib::KernelCurve;

pub(super) fn revolve(
    curve: &NurbsCurve,
    origin: Point3,
    axis: UnitVector3,
    start: Real,
    sweep: Real,
) -> Result<NurbsSurface, CommandError> {
    let length = curve.revolution_parameter_length(origin, axis, sweep)?;
    // Native radial degeneracy thresholds include coordinate magnitude.
    // Work around the axis origin, then restore world placement on the result.
    let zero = Point3::try_new(0., 0., 0.)?;
    // Rhino encodes negative sweeps with increasing circular parameters,
    // starting at the geometric end angle rather than reversing the axis.
    let chart_start = if sweep < 0. { start + sweep } else { start };
    let local = AffineTransform3::from_translation(origin.vector_to(zero)?).then(
        AffineTransform3::try_rotation(zero, axis, chart_start.rem_euclid(std::f64::consts::TAU))?,
    )?;
    let rotated = curve.transformed(local)?;
    let profile = KernelCurve::from_nurbs_for_modeling(&rotated)?;
    let direction = axis.as_vector();
    let degrees = if sweep.abs() == std::f64::consts::TAU {
        360.
    } else {
        sweep.abs().to_degrees()
    };
    Ok(profile
        .revolved_surface(zero, direction, degrees, length)?
        .transformed(AffineTransform3::from_translation(zero.vector_to(origin)?))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_axis_keeps_small_radial_profiles_and_start_angle() {
        let origin = Point3::try_new(1e9, 0., 0.).unwrap();
        let start = Point3::try_new(1e9 + 0.0001, 0., 0.).unwrap();
        let end = Point3::try_new(1e9 + 0.0001, 0., 3.).unwrap();
        let source = LineSegment::try_new(start, end, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
        let output = revolve(
            &source,
            origin,
            axis,
            std::f64::consts::FRAC_PI_2,
            -std::f64::consts::PI,
        )
        .unwrap();
        let first = output
            .evaluate(*output.domain_u().start(), *output.domain_v().start())
            .unwrap();
        let last = output
            .evaluate(*output.domain_u().end(), *output.domain_v().start())
            .unwrap();
        let radius = start.x() - origin.x();
        assert_eq!(first.x(), origin.x());
        assert!((first.y() + radius).abs() < 1e-12);
        assert_eq!(last.x(), origin.x());
        assert!((last.y() - radius).abs() < 1e-12);
    }
}
