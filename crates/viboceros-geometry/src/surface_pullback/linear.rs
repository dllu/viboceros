//! Bounded, primitive-independent straight UV proposals at seams and poles.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct Discovery {
    pub(super) curve: Option<(NurbsCurve2, Real)>,
    /// Reuse the original closest parameters in Hermite endpoint nodes only
    /// when both original images are already within absolute tolerance.
    pub(super) endpoints: Option<[Point2; 2]>,
}

pub(super) fn parameter_line(
    curve: &NurbsCurve,
    endpoints: [Point2; 2],
) -> Result<NurbsCurve2, GeometryError> {
    let domain = curve.domain();
    NurbsCurve2::try_new(
        1,
        endpoints.to_vec(),
        vec![
            *domain.start(),
            *domain.start(),
            *domain.end(),
            *domain.end(),
        ],
    )
}

fn plausible_image(image: Point3, model: Point3, limit: Real) -> bool {
    let scale = image
        .to_array()
        .into_iter()
        .chain(model.to_array())
        .map(Real::abs)
        .fold(1_f64, Real::max);
    image
        .distance_to(model)
        .is_ok_and(|distance| distance <= limit + 4096. * Real::EPSILON * scale)
}

fn station_coordinate(a: Real, b: Real, fraction: Real) -> Option<Real> {
    if a == b {
        return Some(a);
    }
    let value = a.mul_add(1. - fraction, b * fraction);
    let width = b - a;
    // A rounded large-origin coordinate must not stand in for an interior
    // station. Decline this cheap rejection check when interpolation loses
    // fractional position; the exact certificate can still prove the path.
    if !width.is_finite() || (((value - a) / width) - fraction).abs() > 8. * Real::EPSILON {
        None
    } else {
        Some(value)
    }
}

pub(super) fn endpoint_choices(
    surface: &NurbsSurface,
    model: Point3,
    guess: Point2,
    other: Point2,
    middle: Option<Point2>,
    limit: Real,
) -> Result<Vec<Point2>, GeometryError> {
    let u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let mut candidates = vec![guess];
    // At a collapsed edge a coordinate from the other endpoint can select
    // the correct branch without requiring an invertible surface Jacobian.
    for x in [other.x(), u[0], u[1]] {
        candidates.push(Point2::try_new(x, guess.y())?);
    }
    for y in [other.y(), v[0], v[1]] {
        candidates.push(Point2::try_new(guess.x(), y)?);
    }
    for x in u {
        for y in v {
            candidates.push(Point2::try_new(x, y)?);
        }
    }
    // Two singular endpoints may both lose the constant chart coordinate.
    // A single interior closest point can propose it; the proof still decides.
    if let Some(middle) = middle {
        candidates.push(Point2::try_new(middle.x(), guess.y())?);
        candidates.push(Point2::try_new(guess.x(), middle.y())?);
    }
    let mut retained = Vec::with_capacity(candidates.len());
    for p in candidates {
        if !retained.contains(&p)
            && surface
                .evaluate(p.x(), p.y())
                .is_ok_and(|q| plausible_image(q, model, limit))
        {
            retained.push(p);
        }
    }
    Ok(retained)
}

struct Search<'a> {
    surface: &'a NurbsSurface,
    spatial: &'a NurbsCurve,
    limit: Real,
    witnesses: [Point3; 3],
    tried: Vec<[Point2; 2]>,
    certificate: Option<certificate::PullbackCertificate>,
    exhausted: bool,
}

impl Search<'_> {
    fn proposals(
        &mut self,
        first: &[Point2],
        last: &[Point2],
    ) -> Result<Option<(NurbsCurve2, Real)>, GeometryError> {
        if self.exhausted {
            return Ok(None);
        }
        // Translate surface knots and candidate endpoints together before
        // interpolation, so large UV origins do not quantize composition.
        let frame = self
            .surface
            .local_parameter_frame(first.iter().chain(last).map(|p| [p.x(), p.y()]))?;
        for &a in first {
            for &b in last {
                let points = [a, b];
                if self.tried.contains(&points) {
                    continue;
                }
                self.tried.push(points);
                let line = parameter_line(self.spatial, points)?;
                // Three stations reject implausible proposals cheaply. They
                // never establish acceptance. Allow evaluation roundoff here;
                // the exact certificate below always uses the absolute limit.
                let mut plausible = true;
                for (fraction, model) in [0.5, 0.25, 0.75].into_iter().zip(self.witnesses) {
                    let Some(u) = station_coordinate(
                        a.x() - frame.origin[0],
                        b.x() - frame.origin[0],
                        fraction,
                    ) else {
                        continue;
                    };
                    let Some(v) = station_coordinate(
                        a.y() - frame.origin[1],
                        b.y() - frame.origin[1],
                        fraction,
                    ) else {
                        continue;
                    };
                    let Ok(image) = frame.surface.evaluate(u, v) else {
                        plausible = false;
                        break;
                    };
                    if !plausible_image(image, model, self.limit) {
                        plausible = false;
                        break;
                    }
                }
                if !plausible {
                    continue;
                }
                if self.certificate.is_none() {
                    self.certificate = match certificate::PullbackCertificate::with_degree(
                        self.surface,
                        self.spatial,
                        1,
                    ) {
                        Ok(certificate) => certificate,
                        Err(GeometryError::SurfaceCurveCertificateWorkLimit) => None,
                        Err(error) => return Err(error),
                    };
                    if self.certificate.is_none() {
                        self.exhausted = true;
                        return Ok(None);
                    }
                }
                match self.certificate.as_mut().unwrap().curve(&line, self.limit) {
                    Ok(Some(bound)) => return Ok(Some((line, bound))),
                    Ok(None) => {}
                    Err(GeometryError::SurfaceCurveCertificateWorkLimit) => {
                        self.exhausted = true;
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(None)
    }
}

pub(super) fn discover(
    surface: &NurbsSurface,
    spatial: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Discovery, GeometryError> {
    let models = [
        spatial.evaluate(*spatial.domain().start())?,
        spatial.evaluate(*spatial.domain().end())?,
    ];
    let numerical = numerical_pullback_tolerance(tolerance)?;
    let guesses = [
        pullback_parameters(surface, models[0], numerical)?,
        pullback_parameters(surface, models[1], numerical)?,
    ];
    let endpoints = if guesses.into_iter().zip(models).all(|(uv, model)| {
        surface
            .evaluate(uv.x(), uv.y())
            .and_then(|p| p.distance_to(model))
            .is_ok_and(|d| d <= tolerance.absolute())
    }) {
        Some(guesses)
    } else {
        None
    };
    let mut result = Discovery {
        curve: None,
        endpoints,
    };
    let mut first = endpoint_choices(
        surface,
        models[0],
        guesses[0],
        guesses[1],
        None,
        tolerance.absolute(),
    )?;
    let mut last = endpoint_choices(
        surface,
        models[1],
        guesses[1],
        guesses[0],
        None,
        tolerance.absolute(),
    )?;
    if first.is_empty() || last.is_empty() {
        return Ok(result);
    }
    let sampler = spatial.parameter_sampler()?;
    let mut search = Search {
        surface,
        spatial,
        limit: tolerance.absolute(),
        witnesses: [
            sampler.evaluate(0.5)?,
            sampler.evaluate(0.25)?,
            sampler.evaluate(0.75)?,
        ],
        tried: Vec::new(),
        certificate: None,
        exhausted: false,
    };
    result.curve = search.proposals(&first, &last)?;
    if result.curve.is_none() && !search.exhausted && (first.len() > 1 || last.len() > 1) {
        let middle = pullback_parameters(surface, search.witnesses[0], numerical)?;
        first = endpoint_choices(
            surface,
            models[0],
            guesses[0],
            guesses[1],
            Some(middle),
            tolerance.absolute(),
        )?;
        last = endpoint_choices(
            surface,
            models[1],
            guesses[1],
            guesses[0],
            Some(middle),
            tolerance.absolute(),
        )?;
        result.curve = search.proposals(&first, &last)?;
    }
    Ok(result)
}
