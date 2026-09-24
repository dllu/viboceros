//! Apparent crossings of two projected conics. A projective image of a circle
//! or ellipse is a conic; fit its implicit equation in normalized screen
//! coordinates, then isolate roots on the other exact parameterized locus.
use super::{Conic, ConicLocus, SourceChoice, prefer_first_exact};
use crate::object_snap::SnapMetric;
use nalgebra::SMatrix;
use viboceros_document::ObjectId;
use viboceros_geometry::{Point3, Real};

const STATIONS: usize = 128;

struct ImageConic {
    origin: [Real; 2],
    scale: Real,
    coefficients: [Real; 6],
    fit_error: Real,
}

impl ImageConic {
    fn score(&self, image: [Real; 2]) -> Option<Real> {
        let x = (image[0] - self.origin[0]) / self.scale;
        let y = (image[1] - self.origin[1]) / self.scale;
        let [xx, xy, yy, x0, y0, constant] = self.coefficients;
        let value = xx * x * x + xy * x * y + yy * y * y + x0 * x + y0 * y + constant;
        value.is_finite().then_some(value)
    }

    fn gradient(&self, image: [Real; 2]) -> Option<[Real; 2]> {
        let x = (image[0] - self.origin[0]) / self.scale;
        let y = (image[1] - self.origin[1]) / self.scale;
        let [xx, xy, yy, x0, y0, _] = self.coefficients;
        let gradient = [
            (2. * xx * x + xy * y + x0) / self.scale,
            (xy * x + 2. * yy * y + y0) / self.scale,
        ];
        gradient.iter().all(|v| v.is_finite()).then_some(gradient)
    }
}

fn fit(locus: ConicLocus, metric: &impl SnapMetric) -> Option<ImageConic> {
    const FIT: usize = 8;
    let points: [[Real; 2]; FIT] = std::array::from_fn(|i| {
        locus
            .full_point(std::f64::consts::TAU * i as Real / FIT as Real)
            .and_then(|p| metric.offset(p))
            .unwrap_or([Real::NAN; 2])
    });
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let lo: [Real; 2] = std::array::from_fn(|axis| {
        points
            .iter()
            .map(|p| p[axis])
            .fold(Real::INFINITY, Real::min)
    });
    let hi: [Real; 2] = std::array::from_fn(|axis| {
        points
            .iter()
            .map(|p| p[axis])
            .fold(Real::NEG_INFINITY, Real::max)
    });
    let origin = std::array::from_fn(|axis| lo[axis] * 0.5 + hi[axis] * 0.5);
    let scale = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.5;
    if scale == 0. || !scale.is_finite() {
        return None;
    }
    let matrix = SMatrix::<Real, FIT, 6>::from_fn(|row, column| {
        let x = (points[row][0] - origin[0]) / scale;
        let y = (points[row][1] - origin[1]) / scale;
        [x * x, x * y, y * y, x, y, 1.][column]
    });
    let svd = matrix.svd(true, true);
    let singular = svd.singular_values;
    if singular[4] <= 1e-10 || singular[5] > 1e-9 * singular[4] {
        return None;
    }
    let v_t = svd.v_t?;
    let coefficients = std::array::from_fn(|i| v_t[(5, i)]);
    let mut conic = ImageConic {
        origin,
        scale,
        coefficients,
        fit_error: 0.,
    };
    // Reject arbitrary projected callbacks whose image is not a conic.
    for i in 0..FIT {
        let angle = std::f64::consts::TAU * (i as Real + 0.5) / FIT as Real;
        let image = metric.offset(locus.full_point(angle)?)?;
        let error = conic.score(image)?.abs();
        if error > 1e-9 {
            return None;
        }
        conic.fit_error = conic.fit_error.max(error);
    }
    Some(conic)
}

pub(super) fn visit(
    first: Conic,
    second: Conic,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let Some(implicit) = fit(second.locus, metric) else {
        return;
    };
    let sweep = first.locus.sweep();
    let score = |angle: Real| {
        let point = first.locus.full_point(angle)?;
        implicit.score(metric.offset(point)?)
    };
    let derivative = |angle: Real| {
        let point = first.locus.full_point(angle)?;
        let image = metric.offset(point)?;
        let tangent = first.locus.tangent_at_angle(angle)?;
        let screen_tangent = metric.tangent_direction(point, tangent)?;
        let gradient = implicit.gradient(image)?;
        Some(gradient[0] * screen_tangent[0] + gradient[1] * screen_tangent[1])
    };
    let values: [Option<Real>; STATIONS + 1] =
        std::array::from_fn(|i| score(sweep * i as Real / STATIONS as Real));
    let finite: Vec<_> = values.iter().flatten().copied().collect();
    if finite.is_empty() {
        return;
    }
    if finite.iter().all(|v| v.abs() <= 1e-9) {
        coincident_circle_features(first, second, metric, emit);
        return;
    }
    let mut roots = Vec::new();
    let mut tangencies = Vec::new();
    let mut anchors = Vec::new();
    let zero_tolerance = 8. * conic_pair_error(implicit.fit_error);
    for i in 0..STATIONS {
        let a = sweep * i as Real / STATIONS as Real;
        let b = sweep * (i + 1) as Real / STATIONS as Real;
        if values[i].is_some_and(|v| v.abs() <= zero_tolerance) {
            roots.push(a);
            anchors.push(a);
        }
        if let (Some(fa), Some(fb)) = (values[i], values[i + 1])
            && fa.signum() != fb.signum()
        {
            roots.push(bisect(&score, a, b, fa, fb));
        }
        // A tangency is a double root. The derivative changes sign even
        // though the implicit conic score does not.
        if let (Some(da), Some(db)) = (derivative(a), derivative(b))
            && da.signum() != db.signum()
        {
            let stationary = bisect_derivative(&derivative, a, b, da, db);
            if let Some(value) = score(stationary) {
                if value.abs() <= zero_tolerance {
                    roots.push(stationary);
                    tangencies.push(stationary);
                } else {
                    // Two close crossings can lie on opposite sides of an
                    // extremum inside one station interval.
                    if let Some(fa) = values[i]
                        && fa.signum() != value.signum()
                    {
                        roots.push(bisect(&score, a, stationary, fa, value));
                    }
                    if let Some(fb) = values[i + 1]
                        && value.signum() != fb.signum()
                    {
                        roots.push(bisect(&score, stationary, b, value, fb));
                    }
                }
            }
        }
    }
    if values[STATIONS].is_some_and(|v| v.abs() <= zero_tolerance) {
        roots.push(sweep);
        anchors.push(sweep);
    }
    for root in &mut roots {
        for &stationary in anchors.iter().chain(&tangencies) {
            let difference = (*root - stationary).abs();
            let circular_difference = if sweep == std::f64::consts::TAU {
                difference.min(sweep - difference)
            } else {
                difference
            };
            if circular_difference <= 1e-6 {
                *root = stationary;
                break;
            }
        }
    }
    roots.sort_by(Real::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    for angle in roots {
        let Some(point_a) = first.locus.point_on_curve(angle) else {
            continue;
        };
        let Some(image) = metric.offset(point_a) else {
            continue;
        };
        let Some(distance) = metric.captured_offset_distance(image) else {
            continue;
        };
        let Some((point_b, error)) = closest_point(second.locus, image, metric) else {
            continue;
        };
        if error > 1e-9 * metric.capture_radius().max(1.) {
            continue;
        }
        let a = SourceChoice {
            hover_distance: first.hover_distance,
            mesh: false,
            curve_priority: first.locus.priority(false),
            order: first.order,
            point: point_a,
        };
        let b = SourceChoice {
            hover_distance: second.hover_distance,
            mesh: false,
            curve_priority: second.locus.priority(false),
            order: second.order,
            point: point_b,
        };
        let prefer_a =
            tangent_seam_preference(first, second, point_a, point_b, angle, &derivative, metric)
                .or_else(|| near_tangent_circle_preference(first, second))
                .unwrap_or_else(|| prefer_first_exact(a, b, metric));
        let (owner, point) = if prefer_a {
            (first.owner, point_a)
        } else {
            (second.owner, point_b)
        };
        emit(owner, point, distance);
    }
}

fn near_tangent_circle_preference(first: Conic, second: Conic) -> Option<bool> {
    let (ConicLocus::Circle(a), ConicLocus::Circle(b)) = (first.locus, second.locus) else {
        return None;
    };
    let scale = a.radius().max(b.radius());
    if (a.radius() - b.radius()).abs() > 1e-10 * scale {
        return None;
    }
    let center_distance = a.center().distance_to(b.center()).ok()?;
    let sum = a.radius() + b.radius();
    if center_distance <= 0.999 * sum || center_distance >= sum {
        return None;
    }
    let normal_a = a.normal().ok()?;
    let normal_b = b.normal().ok()?;
    if normal_a.as_vector().dot(normal_b.as_vector()).ok()?.abs() < 1. - 1e-10 {
        return None;
    }
    if a.center()
        .vector_to(b.center())
        .ok()?
        .dot(normal_a.as_vector())
        .ok()?
        .abs()
        > 1e-10 * scale
    {
        return None;
    }
    // In measured close circle pairs Rhino assigns both targets to the
    // circle with the lexicographically smaller center, independent of
    // cursor, source order, and parameter frame.
    for (coordinate_a, coordinate_b) in a.center().to_array().into_iter().zip(b.center().to_array())
    {
        if (coordinate_a - coordinate_b).abs() > 1e-9 * scale {
            return Some(coordinate_a < coordinate_b);
        }
    }
    None
}

fn tangent_seam_preference(
    first: Conic,
    second: Conic,
    point_a: Point3,
    point_b: Point3,
    angle: Real,
    derivative: &impl Fn(Real) -> Option<Real>,
    metric: &impl SnapMetric,
) -> Option<bool> {
    if derivative(angle)?.abs() > 1e-8 {
        return None;
    }
    let front_a = metric.frontness(point_a).unwrap_or(0.);
    let front_b = metric.frontness(point_b).unwrap_or(0.);
    if (front_a - front_b).abs() > 64. * Real::EPSILON * front_a.abs().max(front_b.abs()).max(1.) {
        return None;
    }
    let seam_a = first.locus.point_on_curve(0.)?.distance_to(point_a).ok()?;
    let seam_b = second.locus.point_on_curve(0.)?.distance_to(point_b).ok()?;
    let scale = first.locus.radius_bound().max(second.locus.radius_bound());
    let at_a = seam_a <= 1e-9 * scale;
    let at_b = seam_b <= 1e-9 * scale;
    (at_a != at_b).then_some(at_a)
}

fn coincident_circle_features(
    first: Conic,
    second: Conic,
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let (ConicLocus::Circle(a), ConicLocus::Circle(b)) = (first.locus, second.locus) else {
        return;
    };
    let radius = a.radius().max(b.radius());
    let close = 64. * Real::EPSILON * radius.max(1.);
    if (a.radius() - b.radius()).abs() > close
        || !a.center().distance_to(b.center()).is_ok_and(|d| d <= close)
        || a.normal()
            .ok()
            .zip(b.normal().ok())
            .and_then(|(x, y)| x.as_vector().dot(y.as_vector()).ok())
            .is_none_or(|dot| (dot.abs() - 1.).abs() > 64. * Real::EPSILON)
    {
        return;
    }
    let (Ok(qa), Ok(qb)) = (a.quadrants(), b.quadrants()) else {
        return;
    };
    let mut features: Vec<Point3> = Vec::new();
    for point in qa.into_iter().chain(qb) {
        if features
            .iter()
            .any(|&other| point.distance_to(other).is_ok_and(|d| d <= 1e-10 * radius))
        {
            continue;
        }
        features.push(point);
    }
    let seam_a = qa[0];
    let seam_b = qb[0];
    for point in features {
        if let Some(distance) = metric.captured_distance(point) {
            let at_seam_a = point.distance_to(seam_a).is_ok_and(|d| d <= 1e-10 * radius);
            let at_seam_b = point.distance_to(seam_b).is_ok_and(|d| d <= 1e-10 * radius);
            // Rhino reports the other circle at a seam unique to one frame.
            // At the remaining quarter points it favors the frame nearer +X;
            // matching frames fall back to the later source.
            let owner = if at_seam_a != at_seam_b {
                if at_seam_a { second.owner } else { first.owner }
            } else if a == b {
                second.owner
            } else if a.x_axis().x() >= b.x_axis().x() {
                first.owner
            } else {
                second.owner
            };
            emit(owner, point, distance);
        }
    }
}

fn conic_pair_error(fit_error: Real) -> Real {
    fit_error.max(Real::EPSILON)
}

fn bisect(
    score: &impl Fn(Real) -> Option<Real>,
    mut a: Real,
    mut b: Real,
    mut fa: Real,
    _fb: Real,
) -> Real {
    for _ in 0..72 {
        let middle = a * 0.5 + b * 0.5;
        if middle == a || middle == b {
            break;
        }
        let Some(fm) = score(middle) else {
            break;
        };
        if fm.signum() == fa.signum() {
            a = middle;
            fa = fm;
        } else {
            b = middle;
        }
    }
    a * 0.5 + b * 0.5
}

fn bisect_derivative(
    derivative: &impl Fn(Real) -> Option<Real>,
    mut a: Real,
    mut b: Real,
    mut da: Real,
    _db: Real,
) -> Real {
    for _ in 0..72 {
        let middle = a * 0.5 + b * 0.5;
        if middle == a || middle == b {
            break;
        }
        let Some(dm) = derivative(middle) else {
            break;
        };
        if dm.signum() == da.signum() {
            a = middle;
            da = dm;
        } else {
            b = middle;
        }
    }
    a * 0.5 + b * 0.5
}

fn closest_point(
    locus: ConicLocus,
    image: [Real; 2],
    metric: &impl SnapMetric,
) -> Option<(Point3, Real)> {
    let sweep = locus.sweep();
    let score = |angle: Real| {
        let p = locus.point_on_curve(angle)?;
        let q = metric.offset(p)?;
        Some((q[0] - image[0]).hypot(q[1] - image[1]))
    };
    let mut best = (Real::INFINITY, 0);
    for i in 0..=STATIONS {
        let angle = sweep * i as Real / STATIONS as Real;
        if let Some(value) = score(angle)
            && value < best.0
        {
            best = (value, i);
        }
    }
    if !best.0.is_finite() {
        return None;
    }
    let step = sweep / STATIONS as Real;
    let mut left = step.mul_add(best.1 as Real, -step).max(0.);
    let mut right = step.mul_add(best.1 as Real, step).min(sweep);
    let fraction = 0.381_966_011_250_105_1;
    for _ in 0..100 {
        let a = left * (1. - fraction) + right * fraction;
        let b = left * fraction + right * (1. - fraction);
        if a == left || b == right || a == b {
            break;
        }
        if score(a).unwrap_or(Real::INFINITY) <= score(b).unwrap_or(Real::INFINITY) {
            right = b;
        } else {
            left = a;
        }
    }
    let angle = (left + right) * 0.5;
    let refined = score(angle)?;
    let (angle, error) = if refined < best.0 {
        (angle, refined)
    } else {
        (sweep * best.1 as Real / STATIONS as Real, best.0)
    };
    Some((locus.point_on_curve(angle)?, error))
}
