//! Camera-space curve targets, separate from landmark/hover admission.
use super::{ObjectSnapCache, SnapMetric, projected_line, proximity};
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

pub(super) fn line(
    a: Point3,
    b: Point3,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(Point3, Real),
) {
    match projected_line::capture(a, b, metric) {
        projected_line::Capture::Miss => return,
        projected_line::Capture::Point(point) => {
            emit_if_captured(point, metric, emit);
            return;
        }
        projected_line::Capture::Unresolved => {}
    }
    // Numerically unresolved projections retain a bounded visible-locus
    // fallback, never a screen bridging chord.
    let tangent = a.vector_to(b).ok().or_else(|| {
        let a = a.to_array();
        let b = b.to_array();
        let scale = a.into_iter().chain(b).map(Real::abs).fold(0., Real::max);
        Vector3::try_from(std::array::from_fn(|i| b[i] / scale - a[i] / scale)).ok()
    });
    if let Some(tangent) = tangent {
        curve(
            |t| Some((projected_line::interpolate(a, b, t)?, tangent)),
            metric,
            emit,
        );
    }
}

fn emit_if_captured(point: Point3, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    if let Some(distance) = metric.captured_distance(point) {
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
    if let Some((best, _)) = best.filter(|(s, _)| metric.captured_distance(s.point).is_some()) {
        emit(best.point, best.distance);
    }
}

#[cfg(test)]
mod tests;
