//! Reuse SMLib deviation/minimum queries in one normalized working frame.
use super::working_curve::normalized;
use super::*;
use viboceros_geometry::CurveRef;
pub use viboceros_geometry::{CurveDistanceExtrema, CurveDistanceWitness};
unsafe extern "C" {
    fn vb_curve_distance_extrema(
        first: *const NativeCurve,
        second: *const NativeCurve,
        intervals: *const f64,
        tolerance: f64,
        parameters: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
/// Numerical distance extrema on explicit normalized source intervals. The
/// caller owns interval/overlap policy. Upstream maximum candidates and native
/// knot/grid peaks are globally reprojected before ranking. Original images
/// validate the final parameters and nearest-pair property. This is a bounded
/// proposal search, not a certified continuous Hausdorff or global-minimum proof.
pub fn curve_distance_extrema(
    first: CurveRef<'_>,
    second: CurveRef<'_>,
    intervals: [[f64; 2]; 2],
    tolerance: Tolerance,
) -> Result<Option<CurveDistanceExtrema>, Error> {
    for interval in intervals {
        if interval
            .iter()
            .any(|t| !t.is_finite() || !(0. ..=1.).contains(t))
            || interval[0] > interval[1]
        {
            return Err(Error::InvalidOutput);
        }
    }
    if intervals.iter().any(|range| range[0] == range[1]) {
        return Ok(None);
    }
    let refs = [first, second];
    let sources = [first.to_nurbs()?, second.to_nurbs()?];
    let origin = first.start_point()?;
    let mut scale = 0_f64;
    for c in &sources {
        for p in c.control_points() {
            scale = scale.max(origin.distance_to(p.point())?)
        }
    }
    if scale == 0. || !scale.is_finite() {
        return Ok(None);
    }
    if sources.iter().any(|c| c.control_points().len() > 4096) {
        return Err(Error::Kernel(
            "curve distance query exceeds 4096 controls".into(),
        ));
    }
    let mut imported = Vec::new();
    for source in &sources {
        let Some(prepared) = normalized(source, origin, scale)? else {
            return Ok(None);
        };
        imported.push(KernelCurve::from_nurbs(&prepared)?);
    }
    let mut parameters = [0.; 4];
    let input = [
        intervals[0][0],
        intervals[0][1],
        intervals[1][0],
        intervals[1][1],
    ];
    call(|error, capacity| unsafe {
        vb_curve_distance_extrema(
            imported[0].handle.as_ptr(),
            imported[1].handle.as_ptr(),
            input.as_ptr(),
            (tolerance.absolute() / scale).max(64. * f64::EPSILON),
            parameters.as_mut_ptr(),
            error,
            capacity,
        )
    })?;
    let witness = |parameters: [f64; 2]| -> Result<CurveDistanceWitness, Error> {
        let mut native = [0.; 2];
        let mut points = [origin; 2];
        for i in 0..2 {
            if !(intervals[i][0]..=intervals[i][1]).contains(&parameters[i]) {
                return Err(Error::InvalidOutput);
            }
            let t = sources[i].parameter_at(parameters[i])?;
            points[i] = sources[i].evaluate(t)?;
            native[i] = refs[i].parameter_from_nurbs(t)?;
            if refs[i]
                .evaluate(native[i])?
                .distance_bound_to(points[i], tolerance.absolute())?
                .is_none()
            {
                return Err(Error::InvalidOutput);
            }
        }
        Ok(CurveDistanceWitness {
            normalized_parameters: parameters,
            parameters: native,
            points,
            distance: points[0].distance_to(points[1])?,
        })
    };
    let minimum = witness([parameters[2], parameters[3]])?;
    let mut maximum = witness([parameters[0], parameters[1]])?;
    if maximum.distance < minimum.distance {
        if minimum.distance - maximum.distance > tolerance.absolute() {
            return Err(Error::InvalidOutput);
        }
        maximum = minimum.clone();
    }
    // A maximum witness must be a nearest pair in at least one direction.
    // The other curve's full active interval can be broader than the requested
    // range, so independently reproject onto its original trimmed interval.
    let mut pieces = Vec::new();
    for i in 0..2 {
        let domain =
            sources[i].parameter_at(intervals[i][0])?..=sources[i].parameter_at(intervals[i][1])?;
        pieces.push(sources[i].try_trimmed(domain)?);
    }
    let nearest = |w: &CurveDistanceWitness| -> Result<[f64; 2], Error> {
        let mut distances = [0.; 2];
        for i in 0..2 {
            let t = pieces[i].closest_parameter(w.points[1 - i], tolerance)?;
            distances[i] = pieces[i].evaluate(t)?.distance_to(w.points[1 - i])?;
        }
        Ok(distances)
    };
    let directed = nearest(&maximum)?;
    if maximum.distance > directed[0].max(directed[1]) + tolerance.absolute() {
        return Err(Error::InvalidOutput);
    }
    let directed = nearest(&minimum)?;
    if minimum.distance > directed[0].min(directed[1]) + tolerance.absolute() {
        return Err(Error::InvalidOutput);
    }
    Ok(Some(CurveDistanceExtrema {
        intervals,
        maximum,
        minimum,
    }))
}

/// Diagnostic endpoint-projection policy for open curves. Closed pairs retain
/// full domains. This is intentionally separate from the distance producer:
/// Rhino's complete CrvDeviation overlap policy is not established by it.
pub fn projected_curve_intervals(
    first: CurveRef<'_>,
    second: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Option<[[f64; 2]; 2]>, Error> {
    let refs = [first, second];
    let mut intervals = [[0., 1.]; 2];
    if !first.is_closed()? && !second.is_closed()? {
        for i in 0..2 {
            let source = refs[i].to_nurbs()?;
            let other = refs[1 - i];
            let a = source.closest_parameter(other.start_point()?, tolerance)?;
            let b = source.closest_parameter(other.end_point()?, tolerance)?;
            let d = source.domain();
            let length = d.end() - d.start();
            if !length.is_finite() || length <= 0. {
                return Err(Error::InvalidOutput);
            }
            intervals[i] = [
                (a.min(b) - d.start()) / length,
                (a.max(b) - d.start()) / length,
            ];
            if intervals[i][0] >= intervals[i][1] {
                return Ok(None);
            }
        }
    }
    Ok(Some(intervals))
}
#[cfg(test)]
mod tests;
