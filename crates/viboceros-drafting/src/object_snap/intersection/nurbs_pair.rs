//! Exact curve parameters for projected NURBS pairs. A projective camera maps
//! rational NURBS to rational NURBS; fit and validate that camera locally,
//! intersect its two screen-space images, then evaluate the original curves.
use super::{CurvedNurbs, SourceChoice, prefer_first_exact};
use crate::object_snap::SnapMetric;
use nalgebra::SMatrix;
use viboceros_document::ObjectId;
use viboceros_geometry::{
    CurveCurveIntersectionEvent, NurbsCurve, Point3, Real, Tolerance, WeightedPoint3,
};

struct ProjectionFit {
    center: [Real; 3],
    scale: Real,
    image_origin: [Real; 2],
    image_scale: Real,
    matrix: [Real; 12],
}

impl ProjectionFit {
    fn project(&self, point: Point3) -> Option<([Real; 2], Real)> {
        let xyz = point.to_array();
        let q = std::array::from_fn::<_, 4, _>(|i| {
            if i == 3 {
                1.
            } else {
                (xyz[i] - self.center[i]) / self.scale
            }
        });
        let h: [Real; 3] = std::array::from_fn(|row| {
            (0..4)
                .map(|column| self.matrix[4 * row + column] * q[column])
                .sum()
        });
        if h.iter().any(|v| !v.is_finite()) || h[2] == 0. {
            return None;
        }
        let image = [h[0] / h[2], h[1] / h[2]];
        image.iter().all(|v| v.is_finite()).then_some((image, h[2]))
    }

    fn normalized_image(&self, image: [Real; 2]) -> [Real; 2] {
        std::array::from_fn(|i| (image[i] - self.image_origin[i]) / self.image_scale)
    }

    fn curve(&self, curve: &NurbsCurve) -> Option<NurbsCurve> {
        let controls = curve
            .control_points()
            .iter()
            .map(|control| {
                let (image, denominator) = self.project(control.point())?;
                WeightedPoint3::try_new(
                    Point3::try_new(image[0], image[1], 0.).ok()?,
                    control.weight() * denominator,
                )
                .ok()
            })
            .collect::<Option<Vec<_>>>()?;
        NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec()).ok()
    }
}

fn fit(first: &NurbsCurve, second: &NurbsCurve, metric: &impl SnapMetric) -> Option<ProjectionFit> {
    let (mut lo, mut hi) = ([Real::INFINITY; 3], [Real::NEG_INFINITY; 3]);
    for control in first.control_points().iter().chain(second.control_points()) {
        for (axis, value) in control.point().to_array().into_iter().enumerate() {
            lo[axis] = lo[axis].min(value);
            hi[axis] = hi[axis].max(value);
        }
    }
    let center: [Real; 3] = std::array::from_fn(|i| lo[i] * 0.5 + hi[i] * 0.5);
    let radius = (0..3).map(|i| hi[i] - lo[i]).fold(0., Real::max) * 0.5;
    if !radius.is_finite() {
        return None;
    }
    for factor in [1., 0.5, 0.25, 0.125, 0.0625] {
        let scale = radius.max(1.) * factor;
        let points: [[Real; 3]; 8] = std::array::from_fn(|bits| {
            std::array::from_fn(|i| center[i] + if bits & (1 << i) == 0 { -scale } else { scale })
        });
        let images = points.map(|xyz| Point3::try_from(xyz).ok().and_then(|p| metric.offset(p)));
        let Some(images) = images.into_iter().collect::<Option<Vec<_>>>() else {
            continue;
        };
        let image_lo: [Real; 2] =
            std::array::from_fn(|i| images.iter().map(|p| p[i]).fold(Real::INFINITY, Real::min));
        let image_hi: [Real; 2] = std::array::from_fn(|i| {
            images
                .iter()
                .map(|p| p[i])
                .fold(Real::NEG_INFINITY, Real::max)
        });
        let image_origin = std::array::from_fn(|i| image_lo[i] * 0.5 + image_hi[i] * 0.5);
        let image_scale = (image_hi[0] - image_lo[0]).max(image_hi[1] - image_lo[1]) * 0.5;
        if image_scale == 0. || !image_scale.is_finite() {
            continue;
        }
        let mut system = SMatrix::<Real, 16, 12>::zeros();
        for (index, (xyz, image)) in points.into_iter().zip(images).enumerate() {
            let q = std::array::from_fn::<_, 4, _>(|i| {
                if i == 3 {
                    1.
                } else {
                    (xyz[i] - center[i]) / scale
                }
            });
            let screen =
                std::array::from_fn::<_, 2, _>(|i| (image[i] - image_origin[i]) / image_scale);
            for column in 0..4 {
                system[(2 * index, column)] = q[column];
                system[(2 * index + 1, 4 + column)] = q[column];
                system[(2 * index, 8 + column)] = -screen[0] * q[column];
                system[(2 * index + 1, 8 + column)] = -screen[1] * q[column];
            }
        }
        let svd = system.svd(true, true);
        let singular = svd.singular_values;
        if singular[10] <= 1e-10 || singular[11] > 1e-9 * singular[10] {
            continue;
        }
        let v_t = svd.v_t?;
        let sign = if v_t[(11, 11)] < 0. { -1. } else { 1. };
        let matrix = std::array::from_fn(|i| sign * v_t[(11, i)]);
        let fitted = ProjectionFit {
            center,
            scale,
            image_origin,
            image_scale,
            matrix,
        };
        if validate(&fitted, first, metric) && validate(&fitted, second, metric) {
            return Some(fitted);
        }
    }
    None
}

fn validate(fit: &ProjectionFit, curve: &NurbsCurve, metric: &impl SnapMetric) -> bool {
    let Ok(sampler) = curve.parameter_sampler() else {
        return false;
    };
    for span in sampler.spans() {
        for fraction in [0., 0.125, 0.375, 0.5, 0.625, 0.875, 1.] {
            let Some(point) = span.evaluate(fraction).ok() else {
                return false;
            };
            let (Some(observed), Some((predicted, _))) = (metric.offset(point), fit.project(point))
            else {
                return false;
            };
            let expected = fit.normalized_image(observed);
            if (predicted[0] - expected[0]).hypot(predicted[1] - expected[1]) > 1e-9 {
                return false;
            }
        }
    }
    true
}

pub(super) fn visit(
    first: CurvedNurbs<'_>,
    second: CurvedNurbs<'_>,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let Some(fit) = fit(first.curve, second.curve, metric) else {
        return;
    };
    let (Some(projected_a), Some(projected_b)) = (fit.curve(first.curve), fit.curve(second.curve))
    else {
        return;
    };
    let Ok(tolerance) = Tolerance::try_new(1e-7, 1e-10, 1e-10) else {
        return;
    };
    let Ok(events) = projected_a.intersection_events_with_curve(&projected_b, tolerance) else {
        return;
    };
    for event in events {
        let CurveCurveIntersectionEvent::Point(hit) = event else {
            continue; // A shared interval has no isolated interior target.
        };
        let Some((parameter_a, parameter_b)) = refine(
            &projected_a,
            &projected_b,
            hit.first_parameter(),
            hit.second_parameter(),
        ) else {
            continue;
        };
        let (Some(point_a), Some(point_b)) = (
            first.curve.evaluate(parameter_a).ok(),
            second.curve.evaluate(parameter_b).ok(),
        ) else {
            continue;
        };
        let (Some(image_a), Some(image_b)) = (metric.offset(point_a), metric.offset(point_b))
        else {
            continue;
        };
        if (image_a[0] - image_b[0]).hypot(image_a[1] - image_b[1])
            > 1e-8 * metric.capture_radius().max(1.)
        {
            continue;
        }
        let Some(distance) = metric.captured_offset_distance(image_a) else {
            continue;
        };
        let prefer_a = prefer_first_exact(
            SourceChoice {
                hover_distance: first.hover_distance,
                mesh: false,
                curve_priority: 0,
                order: first.order,
                point: point_a,
            },
            SourceChoice {
                hover_distance: second.hover_distance,
                mesh: false,
                curve_priority: 0,
                order: second.order,
                point: point_b,
            },
            metric,
        );
        let (owner, point) = if prefer_a {
            (first.owner, point_a)
        } else {
            (second.owner, point_b)
        };
        emit(owner, point, distance);
    }
}

fn refine(
    first: &NurbsCurve,
    second: &NurbsCurve,
    mut u: Real,
    mut v: Real,
) -> Option<(Real, Real)> {
    let domain_u = first.domain();
    let domain_v = second.domain();
    for _ in 0..20 {
        let (a, da) = first.evaluate_with_derivative(u).ok()?;
        let (b, db) = second.evaluate_with_derivative(v).ok()?;
        let residual = [a.x() - b.x(), a.y() - b.y()];
        let error = residual[0].hypot(residual[1]);
        if error <= 1e-11 {
            return Some((u, v));
        }
        let (aa, bb, cc, dd) = (da.x(), -db.x(), da.y(), -db.y());
        let det = aa * dd - bb * cc;
        if det.abs() <= 1e-14 * (aa.hypot(cc) * bb.hypot(dd)).max(1.) {
            break;
        }
        let du = (-residual[0] * dd + bb * residual[1]) / det;
        let dv = (-aa * residual[1] + residual[0] * cc) / det;
        let mut advanced = false;
        for step in [1., 0.5, 0.25, 0.125, 0.0625, 0.03125] {
            let next_u = (u + step * du).clamp(*domain_u.start(), *domain_u.end());
            let next_v = (v + step * dv).clamp(*domain_v.start(), *domain_v.end());
            let (Some(next_a), Some(next_b)) =
                (first.evaluate(next_u).ok(), second.evaluate(next_v).ok())
            else {
                continue;
            };
            if (next_a.x() - next_b.x()).hypot(next_a.y() - next_b.y()) < error {
                u = next_u;
                v = next_v;
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }
    let a = first.evaluate(u).ok()?;
    let b = second.evaluate(v).ok()?;
    ((a.x() - b.x()).hypot(a.y() - b.y()) <= 1e-10).then_some((u, v))
}
