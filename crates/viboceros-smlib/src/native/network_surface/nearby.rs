//! Nearby contact proposals from an existing exact translational sweep.
use super::*;

pub(super) fn contact(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<[f64; 2]>, Error> {
    // Working curves have positive weights. A separated control hull excludes
    // a nearby pair; use the existing exact distance predicate on each axis.
    let a = first.control_point_bounds();
    let b = second.control_point_bounds();
    for axis in 0..3 {
        let pair = if a.min().to_array()[axis] > b.max().to_array()[axis] {
            Some([a.min().to_array()[axis], b.max().to_array()[axis]])
        } else if b.min().to_array()[axis] > a.max().to_array()[axis] {
            Some([b.min().to_array()[axis], a.max().to_array()[axis]])
        } else {
            None
        };
        if let Some([a, b]) = pair
            && Point3::try_new(a, 0., 0.)?
                .distance_bound_to(Point3::try_new(b, 0., 0.)?, tolerance.absolute())?
                .is_none()
        {
            return Ok(None);
        }
    }
    let negative = second.transformed(AffineTransform3::try_new(
        [[-1., 0., 0.], [0., -1., 0.], [0., 0., -1.]],
        Vector3::try_new(0., 0., 0.)?,
    )?)?;
    let a = KernelCurve::from_nurbs_for_modeling(first)?;
    let b = KernelCurve::from_nurbs_for_modeling(&negative)?;
    // S(u,v) = A(u) - B(v) + B(start). Searching for B(start) therefore
    // proposes a close curve pair without introducing another pair solver.
    let surface = a.translational_sweep(&b)?;
    let query = second.evaluate(*second.domain().start())?;
    let tightened = tolerance.absolute() * 1e-3;
    let numerical = Tolerance::try_new(
        if tightened > 0. {
            tightened
        } else {
            tolerance.absolute()
        },
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let (u, v) = surface.closest_parameters(query, numerical)?;
    if !first.domain().contains(&u) || !second.domain().contains(&v) {
        return Err(Error::InvalidOutput);
    }
    let first_point = first.evaluate(u)?;
    let second_point = second.evaluate(v)?;
    Ok(first_point
        .distance_bound_to(second_point, tolerance.absolute())?
        .map(|_| [u, v]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sweep_proposals_preserve_distinct_curve_parameters_and_reject_separated_hulls() {
        let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let line = |a, b| NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap();
        let a = line(p(0., 0., 5e-8), p(2., 0., 5e-8));
        let b = line(p(0.5, -3., 0.), p(0.5, 1., 0.));
        let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-8).unwrap();
        let [u, v] = contact(&a, &b, tolerance).unwrap().unwrap();
        assert!((u - 0.25).abs() < 1e-10 && (v - 0.75).abs() < 1e-10);
        assert!(
            a.evaluate(u)
                .unwrap()
                .distance_bound_to(b.evaluate(v).unwrap(), tolerance.absolute())
                .unwrap()
                .is_some()
        );
        let far = line(p(0., 0., 2e-7), p(2., 0., 2e-7));
        assert!(contact(&far, &b, tolerance).unwrap().is_none());
    }
}
