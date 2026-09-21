//! Camera-space curve targets, separate from landmark/hover admission.
use super::{ObjectSnapCache, SnapMetric, proximity};
use viboceros_document::Object;
use viboceros_geometry::{CurveRef, Point3, Real, Tolerance, UnitVector3, Vector3};

pub(super) fn visit(
    object: &Object,
    tolerance: Tolerance,
    cache: &mut ObjectSnapCache,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    if let Some(curve) = object.geometry().curve_ref() {
        if let CurveRef::PolyCurve(curve) = curve {
            for segment in curve.segments() {
                analytic(segment.as_ref(), metric, emit);
            }
        } else {
            analytic(curve, metric, emit);
        }
    }
    for feature in cache.geometry_curves(object, tolerance) {
        if feature.bounds.is_some_and(|b| {
            proximity::outside_bounds(b.min().to_array(), b.max().to_array(), metric)
        }) {
            continue;
        }
        let Ok(sampler) = feature.curve.parameter_sampler() else {
            continue;
        };
        for span in sampler.spans() {
            if feature.curve.degree() == 1 && feature.bounds.is_some() {
                if let (Ok(a), Ok(b)) = (span.evaluate(0.), span.evaluate(1.)) {
                    line(a, b, metric, emit);
                }
            } else {
                curve(|t| span.evaluate_with_derivative(t).ok(), metric, emit);
            }
        }
    }
}

fn analytic(source: CurveRef<'_>, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    match source {
        CurveRef::Line(segment) => line(segment.start(), segment.end(), metric, emit),
        CurveRef::Polyline(polyline) => {
            for segment in polyline.segments() {
                line(segment.start(), segment.end(), metric, emit);
            }
        }
        CurveRef::Circle(c) if !proximity::outside_sphere(c.center(), c.radius(), metric) => {
            curve(
                |t| {
                    let angle = std::f64::consts::TAU * t;
                    Some((
                        c.point_at_angle(angle).ok()?,
                        conic_tangent(c.x_axis(), c.y_axis(), 1., 1., angle)?,
                    ))
                },
                metric,
                emit,
            );
        }
        CurveRef::Arc(c) if !proximity::outside_sphere(c.center(), c.radius(), metric) => {
            curve(
                |t| {
                    Some((
                        c.point_at(t).ok()?,
                        conic_tangent(c.x_axis(), c.y_axis(), 1., 1., c.sweep_radians() * t)?,
                    ))
                },
                metric,
                emit,
            );
        }
        CurveRef::Ellipse(c)
            if !proximity::outside_sphere(c.center(), c.radius_x().max(c.radius_y()), metric) =>
        {
            let scale = c.radius_x().max(c.radius_y());
            curve(
                |t| {
                    let angle = std::f64::consts::TAU * t;
                    Some((
                        c.point_at_angle(angle).ok()?,
                        conic_tangent(
                            c.x_axis(),
                            c.y_axis(),
                            c.radius_x() / scale,
                            c.radius_y() / scale,
                            angle,
                        )?,
                    ))
                },
                metric,
                emit,
            );
        }
        CurveRef::PolyCurve(_) => unreachable!("flat leaves"),
        _ => {} // NURBS sources are cached by the owning object; rejected bounds stay cold.
    }
}

fn conic_tangent(
    x: UnitVector3,
    y: UnitVector3,
    rx: Real,
    ry: Real,
    angle: Real,
) -> Option<Vector3> {
    let (s, c) = angle.sin_cos();
    let x = x.as_vector().to_array();
    let y = y.as_vector().to_array();
    Vector3::try_from(std::array::from_fn(|i| -rx * s * x[i] + ry * c * y[i])).ok()
}

pub(super) fn unit_screen(v: [Real; 2]) -> Option<[Real; 2]> {
    let scale = v[0].abs().max(v[1].abs());
    if scale == 0. || !scale.is_finite() {
        return None;
    }
    let v = v.map(|x| x / scale);
    let length = v[0].hypot(v[1]);
    Some(v.map(|x| x / length))
}

fn interpolate(a: Point3, b: Point3, t: Real) -> Option<Point3> {
    if t == 0. {
        return Some(a);
    }
    if t == 1. {
        return Some(b);
    }
    let a = a.to_array();
    let b = b.to_array();
    Point3::try_from(std::array::from_fn(|i| {
        let delta = b[i] - a[i];
        if delta.is_finite() {
            if t <= 0.5 {
                delta.mul_add(t, a[i])
            } else {
                delta.mul_add(t - 1., b[i])
            }
        } else {
            (1. - t) * a[i] + t * b[i]
        }
    }))
    .ok()
}

fn line(a: Point3, b: Point3, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    if let (Some(pa), Some(pb)) = (metric.offset(a), metric.offset(b)) {
        if proximity::segment_distance(pa, pb).is_some_and(|d| d > metric.capture_radius()) {
            return;
        }
        if let Some(point) = visible_line(a, b, pa, pb, metric) {
            emit_if_captured(point, metric, emit);
            return;
        }
    }
    // Clipped endpoints and unresolved projection ratios take the same bounded
    // visible-locus search as curves, never a screen bridging chord.
    let tangent = a.vector_to(b).ok().or_else(|| {
        let a = a.to_array();
        let b = b.to_array();
        let scale = a.into_iter().chain(b).map(Real::abs).fold(0., Real::max);
        Vector3::try_from(std::array::from_fn(|i| b[i] / scale - a[i] / scale)).ok()
    });
    if let Some(tangent) = tangent {
        curve(|t| Some((interpolate(a, b, t)?, tangent)), metric, emit);
    }
}

fn visible_line(
    mut a: Point3,
    mut b: Point3,
    mut pa: [Real; 2],
    mut pb: [Real; 2],
    metric: &impl SnapMetric,
) -> Option<Point3> {
    for orientation in 0..2 {
        let scale = pa.into_iter().chain(pb).map(Real::abs).fold(0., Real::max);
        if scale == 0. {
            return Some(a);
        }
        let na = pa.map(|x| x / scale);
        let nb = pb.map(|x| x / scale);
        let d = [nb[0] - na[0], nb[1] - na[1]];
        let squared = d[0] * d[0] + d[1] * d[1];
        if squared == 0. {
            return Some(a);
        }
        let s = (-(na[0] * d[0] + na[1] * d[1]) / squared).clamp(0., 1.);
        if s == 0. || s == 1. {
            return Some(if s == 0. { a } else { b });
        }
        if metric.is_affine() {
            return interpolate(a, b, s);
        }
        let axis = usize::from(d[1].abs() > d[0].abs());
        let mut lambda = 0.5;
        // Find a well-conditioned image station. A midpoint alone loses the
        // depth ratio when one endpoint is much farther from the camera.
        // Reversing avoids subtracting a tiny model fraction from one.
        for probe in 0..1075 {
            let image = metric.offset(interpolate(a, b, lambda)?)?;
            let mu = (image[axis] / scale - na[axis]) / d[axis];
            if (0.25..=0.75).contains(&mu) {
                let factor = ((1. - s) / s) * (mu / (1. - mu)) * (1. - lambda);
                let t = lambda / (factor + lambda);
                return interpolate(a, b, t);
            }
            if probe == 0 && mu < 0.25 && orientation == 0 {
                break;
            }
            lambda *= 0.5;
            if lambda == 0. {
                return None;
            }
        }
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut pa, &mut pb);
    }
    None
}

fn emit_if_captured(point: Point3, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    if let Some(distance) = metric
        .distance(point)
        .filter(|d| *d <= metric.capture_radius())
    {
        emit(point, distance);
    }
}

#[derive(Clone, Copy)]
struct Station {
    point: Point3,
    distance: Real,
    gradient: Option<Real>,
}

fn station(jet: Option<(Point3, Vector3)>, metric: &impl SnapMetric) -> Option<Station> {
    let (point, tangent) = jet?;
    let offset = metric.offset(point)?;
    let distance = offset[0].hypot(offset[1]);
    if !distance.is_finite() {
        return None;
    }
    let gradient = if distance == 0. || tangent.to_array() == [0.; 3] {
        Some(0.)
    } else {
        metric.tangent_direction(point, tangent).map(|d| {
            // Scaling the cursor offset avoids overflowing a dot product.
            (offset[0] / distance) * d[0] + (offset[1] / distance) * d[1]
        })
    };
    Some(Station {
        point,
        distance,
        gradient,
    })
}

/// Bounded stationarity search, not a certified global solver. Each NURBS span
/// is visited independently; no knot discontinuity supplies a bridging chord.
fn curve(
    jet: impl Fn(Real) -> Option<(Point3, Vector3)>,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    const SAMPLES: usize = 64;
    let mut best: Option<(Station, bool)> = None;
    let mut consider = |candidate: Station, refined: bool| {
        let replace = best.is_none_or(|(old, old_refined)| {
            let rounding = 8. * Real::EPSILON * old.distance.max(candidate.distance);
            candidate.distance < old.distance - rounding
                || (refined && !old_refined && candidate.distance <= old.distance + rounding)
        });
        if replace {
            best = Some((candidate, refined));
        }
    };
    let mut previous = station(jet(0.), metric);
    if let Some(s) = previous {
        consider(s, s.gradient == Some(0.));
    }
    for index in 1..=SAMPLES {
        let t = index as Real / SAMPLES as Real;
        let current = station(jet(t), metric);
        if let Some(s) = current {
            consider(s, s.gradient == Some(0.));
        }
        if let Some((left, right)) = previous.zip(current)
            && left.gradient.is_some_and(|g| g <= 0.)
            && right.gradient.is_some_and(|g| g >= 0.)
            && (left.gradient != Some(0.) || right.gradient != Some(0.))
        {
            let mut a = (index - 1) as Real / SAMPLES as Real;
            let mut b = t;
            let mut candidate = None;
            for _ in 0..96 {
                let middle = a + (b - a) * 0.5;
                if middle == a || middle == b {
                    break;
                }
                let Some(s) = station(jet(middle), metric) else {
                    candidate = None;
                    break;
                };
                let Some(g) = s.gradient else {
                    candidate = None;
                    break;
                };
                candidate = Some(s);
                if g == 0. {
                    break;
                }
                if g < 0. { a = middle } else { b = middle }
            }
            if let Some(candidate) = candidate {
                consider(candidate, true);
            }
        }
        previous = current;
    }
    if let Some((best, _)) = best.filter(|(s, _)| s.distance <= metric.capture_radius()) {
        emit(best.point, best.distance);
    }
}

#[cfg(test)]
mod tests;
