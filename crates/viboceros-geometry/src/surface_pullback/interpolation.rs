//! Certified derivative-free fitting at singular nodes and ambiguous endpoints.
use super::*;
use crate::NurbsCurveParameterSampler;

#[cfg(test)]
mod tests;

struct Fitter<'a> {
    surface: &'a NurbsSurface,
    original: &'a NurbsSurface,
    source: &'a NurbsCurve,
    sampler: NurbsCurveParameterSampler<'a>,
    origin: [Real; 2],
    numerical: Tolerance,
    tolerance: Tolerance,
    certificate: certificate::PullbackCertificate,
    segments: Vec<PullbackSegment>,
}

fn blend(points: &[Point2], weights: &[Real]) -> Result<Point2, GeometryError> {
    // Center first so fitting in a translated chart does not lose small offsets.
    let anchor = points[0];
    let coordinate = |get: fn(Point2) -> Real| {
        points
            .iter()
            .zip(weights)
            .skip(1)
            .fold(get(anchor), |sum, (p, w)| {
                w.mul_add(get(*p) - get(anchor), sum)
            })
    };
    Point2::try_new(coordinate(Point2::x), coordinate(Point2::y))
}

impl Fitter<'_> {
    fn node(&self, fraction: Real) -> Result<Point2, GeometryError> {
        pullback_parameters(
            self.surface,
            self.sampler.evaluate(fraction)?,
            self.numerical,
        )
    }

    fn restore(&self, p: Point2) -> Result<Point2, GeometryError> {
        Point2::try_new(p.x() + self.origin[0], p.y() + self.origin[1])
    }

    fn endpoint_candidates(
        &self,
        fraction: Real,
        fixed: Option<Point2>,
        extrapolated: Point2,
    ) -> Result<Vec<Point2>, GeometryError> {
        if let Some(p) = fixed {
            return Ok(vec![p]);
        }
        let model = self.sampler.evaluate(fraction)?;
        let guess = self.node(fraction)?;
        let u = [
            *self.surface.domain_u().start(),
            *self.surface.domain_u().end(),
        ];
        let v = [
            *self.surface.domain_v().start(),
            *self.surface.domain_v().end(),
        ];
        let extrapolated = Point2::try_new(
            snap_domain_roundoff(extrapolated.x(), u),
            snap_domain_roundoff(extrapolated.y(), v),
        )?;
        let mut points = linear::endpoint_choices(
            self.surface,
            model,
            guess,
            extrapolated,
            Some(extrapolated),
            self.tolerance.absolute(),
        )?;
        if self.surface.domain_u().contains(&extrapolated.x())
            && self.surface.domain_v().contains(&extrapolated.y())
            && self
                .surface
                .evaluate(extrapolated.x(), extrapolated.y())?
                .distance_to(model)?
                <= self.tolerance.absolute()
            && !points.contains(&extrapolated)
        {
            points.insert(0, extrapolated);
        }
        Ok(points)
    }

    fn append(
        &mut self,
        interval: [Real; 2],
        endpoints: [Option<Point2>; 2],
        depth: usize,
    ) -> Result<(), GeometryError> {
        let [start, end] = interval;
        let fraction = |t: Real| start.mul_add(1. - t, end * t);
        let interior = [
            self.node(fraction(0.2))?,
            self.node(fraction(0.4))?,
            self.node(fraction(0.6))?,
            self.node(fraction(0.8))?,
        ];
        let first =
            self.endpoint_candidates(start, endpoints[0], blend(&interior, &[4., -6., 4., -1.])?)?;
        let last =
            self.endpoint_candidates(end, endpoints[1], blend(&interior, &[-1., 4., -6., 4.])?)?;
        let third = self.node(fraction(1. / 3.))?;
        let two_thirds = self.node(fraction(2. / 3.))?;
        for &a in &first {
            for &b in &last {
                let points = [a, third, two_thirds, b];
                let controls = [
                    a,
                    blend(&points, &[-5. / 6., 3., -1.5, 1. / 3.])?,
                    blend(&points, &[1. / 3., -1.5, 3., -5. / 6.])?,
                    b,
                ];
                let native = controls
                    .map(|p| self.restore(p))
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()?;
                let native: [Point2; 4] = native.try_into().unwrap();
                if self.certificate.fractional_segment(
                    interval,
                    native,
                    self.tolerance.absolute(),
                )? {
                    if self.segments.len() == MAX_PULLBACK_SEGMENTS {
                        return Err(GeometryError::TooManySurfacePullbackControlPoints {
                            maximum: MAX_CURVE_DIVISION_POINTS,
                        });
                    }
                    self.segments.push(PullbackSegment {
                        start: self.source.parameter_at(start)?,
                        end: self.source.parameter_at(end)?,
                        controls: native,
                    });
                    return Ok(());
                }
            }
        }
        if depth == MAX_PULLBACK_SUBDIVISION_DEPTH {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let middle = start.mul_add(0.5, end * 0.5);
        let point = self.node(middle)?;
        self.append([start, middle], [endpoints[0], Some(point)], depth + 1)?;
        self.append([middle, end], [Some(point), endpoints[1]], depth + 1)
    }
}

pub(super) fn fit(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    endpoints: Option<[Point2; 2]>,
    tolerance: Tolerance,
) -> Result<(NurbsCurve2, Real), GeometryError> {
    let frame =
        surface.local_parameter_frame(endpoints.into_iter().flatten().map(|p| [p.x(), p.y()]))?;
    let endpoints = endpoints
        .map(|p| -> Result<_, GeometryError> {
            Ok([
                Point2::try_new(p[0].x() - frame.origin[0], p[0].y() - frame.origin[1])?,
                Point2::try_new(p[1].x() - frame.origin[0], p[1].y() - frame.origin[1])?,
            ])
        })
        .transpose()?;
    let certificate = certificate::PullbackCertificate::new(surface, source)?.ok_or(
        GeometryError::SurfacePullbackDidNotConverge {
            tolerance: tolerance.absolute(),
        },
    )?;
    let mut fitter = Fitter {
        surface: &frame.surface,
        original: surface,
        source,
        sampler: source.parameter_sampler()?,
        origin: frame.origin,
        numerical: numerical_pullback_tolerance(tolerance)?,
        tolerance,
        certificate,
        segments: Vec::new(),
    };
    fitter.append([0., 1.], endpoints.map_or([None; 2], |p| p.map(Some)), 0)?;
    // Restoring native knot times and controls can round the assembled spline.
    // Qualify that actual returned curve, never only the fractional proposals.
    let uv = piecewise_cubic(&fitter.segments)?;
    let bound = fitter
        .original
        .parameter_curve_deviation_bound(&uv, source, tolerance.absolute())?
        .ok_or(GeometryError::SurfacePullbackDidNotConverge {
            tolerance: tolerance.absolute(),
        })?;
    Ok((uv, bound))
}
