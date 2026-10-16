//! Common simple-knot cubic families using native constrained/geometric fits.
use super::*;
use viboceros_geometry::{CurveFitStation, CurveRef, ParameterSide};
mod clock;

/// One oriented open source with ordered original parameters and shared unit
/// output stations, including both natural endpoints. Targets can move within
/// the fit tolerance; endpoint targets keep their original positions within
/// numerical tolerance.
pub struct GordonCurveFitInput<'a> {
    pub curve: &'a NurbsCurve,
    pub stations: &'a [CurveFitStation],
}

pub struct GordonCurveFamilyFit {
    pub curves: Vec<NurbsCurve>,
    /// Maximum native point-cloud projection error, scaled to model units.
    /// This is a sampled diagnostic in the private working frame, not a
    /// continuous original-curve certificate or a rounding-error bound.
    pub maximum_sampled_point_distance: f64,
}

unsafe extern "C" {
    fn vb_network_curve_fit(
        points: *const f64,
        parameters: *const f64,
        point_count: usize,
        fixed: *const usize,
        fixed_count: usize,
        knots: *const f64,
        knot_count: usize,
        derivatives: *const f64,
        tolerance: f64,
        maximum_error: *mut f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

fn invalid(message: &str) -> Error {
    Error::Kernel(format!("Gordon family fit: {message}"))
}

/// Fit 1–32 open curves to exactly the same simple-knot cubic basis. Each
/// network interval has the supplied number of uniform spans (at least four);
/// at most 128 fitted controls and 2,048 input controls are supported. Position
/// constraints retain their prescribed common stations. Existing native least
/// squares and bounded geometric refitting supply the control points.
///
/// Arc-length samples of private trimmed proposals drive fitting through a
/// monotone quintic clock with shared derivatives at contacts. Projection
/// errors are sampled, and exact stored-curve proofs additionally qualify the
/// prescribed positions after world restoration. A downstream surface must
/// separately certify every complete pre-fit original image. No NetworkSrf
/// interior, source topology or general performance qualification is made.
pub fn fit_gordon_curve_family(
    inputs: &[GordonCurveFitInput<'_>],
    interval_spans: &[usize],
    fit_tolerance: f64,
    numerical: Tolerance,
) -> Result<GordonCurveFamilyFit, Error> {
    if !(1..=32).contains(&inputs.len())
        || !fit_tolerance.is_finite()
        || fit_tolerance <= 0.
        || inputs
            .iter()
            .map(|i| i.curve.control_points().len())
            .sum::<usize>()
            > 2048
    {
        return Err(invalid("invalid family, tolerance or control budget"));
    }
    let stations = inputs[0].stations;
    if !(2..=32).contains(&stations.len())
        || interval_spans.len() + 1 != stations.len()
        || interval_spans.iter().any(|&n| !(4..=124).contains(&n))
        || interval_spans.iter().sum::<usize>() > 125
        || stations[0].fit_parameter != 0.
        || stations.last().unwrap().fit_parameter != 1.
        || stations.iter().any(|s| !s.fit_parameter.is_finite())
        || stations
            .windows(2)
            .any(|p| p[0].fit_parameter >= p[1].fit_parameter)
    {
        return Err(invalid("invalid common stations or span budget"));
    }
    let knots = common_knots(stations, interval_spans)?;
    let origin = inputs[0]
        .curve
        .evaluate(*inputs[0].curve.domain().start())?;
    let diameter = inputs
        .iter()
        .flat_map(|i| i.curve.control_points())
        .try_fold(0_f64, |m, p| {
            Ok::<_, Error>(m.max(origin.distance_to(p.point())?))
        })?;
    if !diameter.is_finite() || diameter <= fit_tolerance {
        return Err(invalid("collapsed working frame"));
    }
    // A bounded binary magnification keeps the native fixed absolute stopping
    // tolerance useful. Global tolerances and final admission limits never move.
    let scale = if diameter / 1024. > 0. {
        diameter / 1024.
    } else {
        diameter
    };
    let local_numerical = Tolerance::try_new(
        numerical.absolute() / scale,
        numerical.relative(),
        numerical.angular(),
    )?;
    let restore = AffineTransform3::try_new(
        [[scale, 0., 0.], [0., scale, 0.], [0., 0., scale]],
        Vector3::try_from(origin.to_array())?,
    )?;
    let mut curves = Vec::with_capacity(inputs.len());
    let mut maximum = 0_f64;
    for input in inputs {
        validate_source(input, stations, fit_tolerance, numerical)?;
        let local = working_curve::normalized(input.curve, origin, scale)?
            .ok_or_else(|| invalid("unsupported original basis or weights"))?;
        if local.evaluate(0.)?.distance_to(local.evaluate(1.)?)? <= local_numerical.absolute() {
            return Err(invalid("requires open noncollapsed sources"));
        }
        let source_parameters = input
            .stations
            .iter()
            .map(|s| fraction(input.curve, s.source_parameter))
            .collect::<Vec<_>>();
        if source_parameters.windows(2).any(|p| p[0] >= p[1]) {
            return Err(invalid("source chart collapsed stations"));
        }
        let mut points = vec![];
        let mut parameters = vec![];
        let mut fixed = vec![];
        let mut lengths = vec![];
        let mut pieces = vec![];
        for j in 0..interval_spans.len() {
            let piece = trim_curve(&local, source_parameters[j]..=source_parameters[j + 1])?;
            lengths.push(piece.length(local_numerical)?);
            pieces.push(piece);
        }
        let clock = clock::Clock::new(stations, &lengths)?;
        for (j, &spans) in interval_spans.iter().enumerate() {
            let count = spans * 8;
            let samples = CurveRef::NurbsCurve(&pieces[j]).sample_equal_length_points(
                count,
                true,
                local_numerical,
            )?;
            fixed.push(parameters.len());
            for (i, point) in samples.into_iter().take(count).enumerate() {
                let parameter = if i == 0 {
                    stations[j].fit_parameter
                } else {
                    clock.pieces[j].evaluate(i as f64 / count as f64)?.x()
                };
                parameters.push(parameter);
                let p = if i == 0 {
                    to_local(input.stations[j].point, origin, scale)?
                } else {
                    point
                };
                points.extend(p.to_array());
            }
        }
        fixed.push(parameters.len());
        parameters.push(1.);
        points.extend(to_local(input.stations.last().unwrap().point, origin, scale)?.to_array());
        let mut derivatives = Vec::with_capacity(6);
        for (t, j) in [(0., 0), (1., interval_spans.len())] {
            let derivative = local.evaluate_with_derivative(t)?.1;
            let length = derivative.length()?;
            if length == 0. {
                return Err(invalid("stationary source endpoint"));
            }
            derivatives.extend(
                derivative
                    .scaled(1. / (clock.slopes[j] * length))?
                    .to_array(),
            );
        }
        let mut pointer = std::ptr::null_mut();
        let mut error_distance = 0.;
        call(|error, capacity| unsafe {
            vb_network_curve_fit(
                points.as_ptr(),
                parameters.as_ptr(),
                parameters.len(),
                fixed.as_ptr(),
                fixed.len(),
                knots.as_ptr(),
                knots.len(),
                derivatives.as_ptr(),
                fit_tolerance / scale,
                &mut error_distance,
                &mut pointer,
                error,
                capacity,
            )
        })?;
        let candidate = brep::curve_from_native(pointer)?.transformed(restore)?;
        if candidate.degree() != 3
            || candidate.knots() != knots
            || candidate.control_points().iter().any(|p| p.weight() != 1.)
        {
            return Err(invalid("native fit changed the shared polynomial basis"));
        }
        if !error_distance.is_finite() || error_distance * scale > fit_tolerance {
            return Err(invalid(&format!(
                "sampled point distance {} exceeds {fit_tolerance}",
                error_distance * scale
            )));
        }
        let positions = input
            .stations
            .iter()
            .map(|s| (s.fit_parameter, ParameterSide::Right, s.point))
            .collect::<Vec<_>>();
        if !candidate.positions_within_distance(&positions, numerical.absolute())? {
            return Err(invalid("stored-curve position constraints did not certify"));
        }
        maximum = maximum.max(error_distance * scale);
        curves.push(candidate);
    }
    Ok(GordonCurveFamilyFit {
        curves,
        maximum_sampled_point_distance: maximum,
    })
}

fn common_knots(stations: &[CurveFitStation], spans: &[usize]) -> Result<Vec<f64>, Error> {
    let mut knots = vec![0.; 4];
    for (j, &count) in spans.iter().enumerate() {
        for i in 1..=count {
            if j + 1 == spans.len() && i == count {
                break;
            }
            let t = if i == count {
                stations[j + 1].fit_parameter
            } else {
                (i as f64 / count as f64).mul_add(
                    stations[j + 1].fit_parameter - stations[j].fit_parameter,
                    stations[j].fit_parameter,
                )
            };
            if t <= *knots.last().unwrap() || t >= 1. {
                return Err(invalid("common knot span collapsed"));
            }
            knots.push(t);
        }
    }
    knots.extend([1.; 4]);
    Ok(knots)
}
fn fraction(curve: &NurbsCurve, t: f64) -> f64 {
    let domain = curve.domain();
    let (a, b) = (*domain.start(), *domain.end());
    if t == a {
        return 0.;
    }
    if t == b {
        return 1.;
    }
    if (b - a).is_finite() {
        (t - a) / (b - a)
    } else {
        let s = a.abs().max(b.abs());
        (t / s - a / s) / (b / s - a / s)
    }
}
fn to_local(point: Point3, origin: Point3, scale: f64) -> Result<Point3, Error> {
    Ok(Point3::try_new(
        (point.x() - origin.x()) / scale,
        (point.y() - origin.y()) / scale,
        (point.z() - origin.z()) / scale,
    )?)
}
fn validate_source(
    input: &GordonCurveFitInput<'_>,
    common: &[CurveFitStation],
    fit: f64,
    numerical: Tolerance,
) -> Result<(), Error> {
    let curve = input.curve;
    let stations = input.stations;
    if curve.degree() > 16
        || curve.control_points().len() > 128
        || stations.len() != common.len()
        || stations
            .iter()
            .zip(common)
            .any(|(a, b)| a.fit_parameter != b.fit_parameter)
        || stations.first().unwrap().source_parameter != *curve.domain().start()
        || stations.last().unwrap().source_parameter != *curve.domain().end()
        || stations.iter().any(|s| !s.source_parameter.is_finite())
        || stations
            .windows(2)
            .any(|p| p[0].source_parameter >= p[1].source_parameter)
    {
        return Err(invalid("invalid source stations or basis budget"));
    }
    let positions = stations
        .iter()
        .map(|s| (s.source_parameter, ParameterSide::Right, s.point))
        .collect::<Vec<_>>();
    if !curve.positions_within_distance(&positions, fit)?
        || !curve.positions_within_distance(
            &[positions[0], *positions.last().unwrap()],
            numerical.absolute(),
        )?
    {
        return Err(invalid("targets move beyond source/endpoint tolerance"));
    }
    Ok(())
}
