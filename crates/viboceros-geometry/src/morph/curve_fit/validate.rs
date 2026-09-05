use super::*;

pub(super) struct Errors {
    pub deviation: Real,
    pub refinements: Vec<(usize, Real)>,
}

pub(super) fn errors(
    point_at: &mut impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
    fitted: &NurbsCurve,
    breaks: &[Break],
    fractions: &[Real],
    threshold: Real,
) -> Result<Errors, GeometryError> {
    let mut result = Errors {
        deviation: 0.0,
        refinements: Vec::new(),
    };
    for (index, interval) in breaks.windows(2).enumerate() {
        let (start, end) = (interval[0].parameter, interval[1].parameter);
        let mut error: Real = 0.0;
        for &fraction in fractions {
            let t = stable_lerp(start, end, fraction)?;
            let side = if t == end {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let exact = point_at(t, side)?;
            let actual = fitted.evaluate_on_side(t, side)?;
            error = error.max(exact.distance_to(actual)?);
        }
        result.deviation = result.deviation.max(error);
        if error > threshold {
            result.refinements.push((index, error));
        }
    }
    Ok(result)
}
