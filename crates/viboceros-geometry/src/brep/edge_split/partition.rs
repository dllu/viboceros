use super::*;

/// Full-multiplicity cuts can slice controls directly, avoiding the quadratic
/// control copying caused by repeatedly splitting a shrinking polygon curve.
pub(super) fn subcurve(
    curve: &NurbsCurve,
    interval: RangeInclusive<Real>,
    budget: &mut Budget,
) -> Result<NurbsCurve, GeometryError> {
    crate::parameter::check_trim_interval(&interval, curve.domain())?;
    let (start, end) = (*interval.start(), *interval.end());
    let knots = curve.knots();
    let degree = curve.degree();
    let a = knots.partition_point(|&k| k < start);
    let b = knots.partition_point(|&k| k <= start);
    let c = knots.partition_point(|&k| k < end);
    let d = knots.partition_point(|&k| k <= end);
    if b - a >= degree && d - c >= degree {
        let first = if b - a == degree {
            a.checked_sub(1)
        } else {
            Some(a)
        };
        let last = c;
        if let Some(first) = first
            && first < last
            && last <= curve.control_points().len()
        {
            budget.charge(last - first)?;
            let mut result_knots = vec![start; degree + 1];
            result_knots.extend_from_slice(&knots[b..c]);
            result_knots.extend(std::iter::repeat_n(end, degree + 1));
            return NurbsCurve::try_new_rational(
                degree,
                curve.control_points()[first..last].to_vec(),
                result_knots,
            );
        }
    }
    budget.charge(curve.control_points().len().saturating_mul(4))?;
    curve.try_trimmed(interval)
}
