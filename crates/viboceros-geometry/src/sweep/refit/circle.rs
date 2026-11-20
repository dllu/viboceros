//! Four equal cubic arcs with C1 joins at the circle's rotated quadrants.
use super::*;
use std::f64::consts::TAU;

/// The standard quarter cubic interpolates its arc at 0, 1/2 and 1 of
/// each span. Its maximum relative radial error is less than 0.000273.
const RADIAL_ERROR: Real = 0.000273;

pub(super) fn fit(
    source: &NurbsCurve,
    limit: Real,
    single: bool,
) -> Result<Option<NurbsCurve>, GeometryError> {
    if source.degree() != 2 {
        return Ok(None);
    }
    let recognition = Tolerance::try_new(
        Tolerance::DEFAULT.absolute().min(limit * 0.001),
        Tolerance::DEFAULT.relative(),
        Tolerance::DEFAULT.angular(),
    )?;
    let Some(center) = source.circular_center(recognition)? else {
        return Ok(None);
    };
    let domain = source.domain();
    let start = source.evaluate(*domain.start())?;
    let radial = center.vector_to(start)?;
    let radius = radial.length()?;
    if radius * RADIAL_ERROR > limit - recognition.absolute() {
        return Ok(None);
    }
    let tangent = source
        .tangent_at_on_side(*domain.start(), ParameterSide::Right)?
        .as_vector();
    let x = radial.normalized_nonzero()?.as_vector();
    let normal = x.cross(tangent)?.normalized_nonzero()?.as_vector();
    let y = normal.cross(x)?;
    // A circular locus alone can contain multiple traversals or reversals.
    // Positive rational quadratics sweep at most pi in a single Bézier span.
    // Verify their ordered sweep before replacing the source parameterization.
    let mut turn = 0.;
    for span in source.try_bezier_spans()? {
        if span.control_points().iter().any(|p| p.weight() <= 0.) {
            return Ok(None);
        }
        let domain = span.domain();
        let a = center.vector_to(span.evaluate(*domain.start())?)?;
        let b = center.vector_to(span.evaluate(*domain.end())?)?;
        let step = normal.dot(a.cross(b)?)?.atan2(a.dot(b)?);
        if step <= 0. {
            return Ok(None);
        }
        turn += step;
    }
    if (turn - TAU).abs() > recognition.angular() {
        return Ok(None);
    }
    let circumference = TAU * radius;
    let input_width = domain.end() - domain.start();
    let clock = if !single {
        1.
    } else if (input_width - circumference).abs() <= circumference * (64. * Real::EPSILON) {
        // The native analytic circle carries its exact arc-length chart.
        circumference
    } else {
        fit_length(source)?
    };
    let handle = (4. / 3.) * (std::f64::consts::SQRT_2 - 1.) * radius;
    let axes = [x, y, x.scaled(-1.)?, y.scaled(-1.)?];
    let mut controls = vec![start, start.translated(y.scaled(handle)?)?];
    let mut knots = vec![0.; 4];
    for quadrant in 1..4 {
        let vertex = center.translated(axes[quadrant].scaled(radius)?)?;
        let offset = axes[(quadrant + 1) % 4].scaled(handle)?;
        controls.extend([
            vertex.translated(offset.scaled(-1.)?)?,
            vertex.translated(offset)?,
        ]);
        knots.extend([(quadrant as Real / 4.) * clock; 2]);
    }
    controls.extend([start.translated(y.scaled(-handle)?)?, start]);
    knots.extend([clock; 4]);
    NurbsCurve::try_new(3, controls, knots).map(Some)
}
