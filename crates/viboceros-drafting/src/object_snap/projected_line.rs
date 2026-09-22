//! Straight-locus projection, clipping, and inverse perspective interpolation.
//! Curved searches must not approximate straight lines by uniform stations.
use super::SnapMetric;
use viboceros_geometry::{Point3, Real};

#[cfg(test)]
mod observed_tests;
#[cfg(test)]
mod tests;

pub(super) enum Capture {
    Point(Point3),
    Miss,
    Unresolved,
}

struct Segment {
    a: Point3,
    b: Point3,
    pa: [Real; 2],
    pb: [Real; 2],
}

/// Project the visible interval. With one visible endpoint, bisection in model
/// coordinates resolves even a tiny visible fraction without storing 1 - tiny.
/// Both unprojectable endpoints remain unresolved: numerical projection limits
/// can hide finite interior points, unlike rejection by a single clipping plane.
fn visible_segment(a: Point3, b: Point3, metric: &impl SnapMetric) -> Option<Segment> {
    let pa = metric.offset(a);
    let pb = metric.offset(b);
    let (inside, outside, image) = match (pa, pb) {
        (Some(pa), Some(pb)) => return Some(Segment { a, b, pa, pb }),
        (Some(pa), None) => (a, b, pa),
        (None, Some(pb)) => (b, a, pb),
        (None, None) => return None,
    };
    let (mut lo, mut hi, mut projected) = (inside, outside, image);
    // Covers the finite binary64 exponent range and significand. Usually the
    // endpoints become adjacent in a few dozen steps; no epsilon clips away a
    // thin visible interval. Each retained endpoint actually projects.
    for _ in 0..2200 {
        let middle = lo.midpoint(hi).ok()?;
        if middle == lo || middle == hi {
            break;
        }
        if let Some(p) = metric.offset(middle) {
            lo = middle;
            projected = p;
        } else {
            hi = middle;
        }
    }
    Some(Segment {
        a: inside,
        b: lo,
        pa: image,
        pb: projected,
    })
}

#[cfg(test)]
pub(super) fn distance(a: Point3, b: Point3, metric: &impl SnapMetric) -> Option<Real> {
    let [x, y] = closest_offset(a, b, metric)?;
    let distance = x.hypot(y);
    distance.is_finite().then_some(distance)
}

/// Hover needs the projected locus point, not a rounded absolute world point.
/// Keeping this offset separate preserves sub-ULP cursor motion at large origins.
pub(super) fn closest_offset(a: Point3, b: Point3, metric: &impl SnapMetric) -> Option<[Real; 2]> {
    let segment = visible_segment(a, b, metric)?;
    Some(segment_offset(segment.pa, segment.pb))
}

pub(super) fn capture(a: Point3, b: Point3, metric: &impl SnapMetric) -> Capture {
    capture_with_policy(a, b, metric, false)
}

/// Mesh Near uses measured endpoint-depth weighting when neither endpoint is
/// inside the square snap aperture. Both inside selects the screen-nearest
/// endpoint; exactly one inside uses ordinary screen proximity. Curve Near
/// remains screen-Euclidean independently of endpoint admission.
pub(super) fn capture_mesh(a: Point3, b: Point3, metric: &impl SnapMetric) -> Capture {
    capture_with_policy(a, b, metric, true)
}

fn capture_with_policy(a: Point3, b: Point3, metric: &impl SnapMetric, mesh: bool) -> Capture {
    let Some(segment) = visible_segment(a, b, metric) else {
        return Capture::Unresolved;
    };
    let radius = metric.capture_radius();
    // The projected visible locus is a segment. Reject separated screen bounds
    // before norms and inverse projection, particularly in large line scenes.
    if (0..2).any(|i| {
        segment.pa[i].min(segment.pb[i]) > radius || segment.pa[i].max(segment.pb[i]) < -radius
    }) {
        return Capture::Miss;
    }
    // A circumscribed circle is only a broad phase: the old inscribed-circle
    // cutoff incorrectly rejected targets near square-aperture corners.
    if segment_distance(segment.pa, segment.pb)
        .is_some_and(|distance| distance > radius.hypot(radius).next_up())
    {
        return Capture::Miss;
    }
    let inside = [segment.pa, segment.pb].map(|p| p.iter().all(|x| x.abs() <= radius));
    if mesh && segment.a == a && segment.b == b && inside.into_iter().all(|v| v) {
        // Retained GetPoint and public per-line picking controls distinguish
        // this endpoint rule from the screen-nearest interior point. Exact
        // screen ties retain the first endpoint, even at different depths.
        // A synthesized clipping-plane point is not an original wire endpoint.
        let da = segment.pa[0].hypot(segment.pa[1]);
        let db = segment.pb[0].hypot(segment.pb[1]);
        return Capture::Point(if db < da { segment.b } else { segment.a });
    }
    let point = if mesh && !metric.is_affine() && !inside.into_iter().any(|v| v) {
        visible_mesh_line(segment, metric)
    } else {
        visible_line(segment.a, segment.b, segment.pa, segment.pb, metric)
    };
    point.map_or(Capture::Unresolved, Capture::Point)
}

fn visible_mesh_line(mut segment: Segment, metric: &impl SnapMetric) -> Option<Point3> {
    // Recover relative homogeneous endpoint depths from an interior station.
    // The projection callback deliberately exposes neither a camera nor W.
    // Halving toward the shallow endpoint keeps the image fraction conditioned.
    for orientation in 0..2 {
        let Segment { a, b, pa, pb } = segment;
        let scale = pa.into_iter().chain(pb).map(Real::abs).fold(0., Real::max);
        if scale == 0. {
            return Some(a);
        }
        let na = pa.map(|x| x / scale);
        let nb = pb.map(|x| x / scale);
        let delta = [nb[0] - na[0], nb[1] - na[1]];
        let axis = usize::from(delta[1].abs() > delta[0].abs());
        if delta[axis] == 0. {
            return Some(a);
        }
        let mut lambda = 0.5;
        for probe in 0..1075 {
            let image = metric.offset(interpolate(a, b, lambda)?)?;
            let mu = (image[axis] / scale - na[axis]) / delta[axis];
            if (0.25..=0.75).contains(&mu) {
                let wa = lambda * (1. - mu);
                let wb = (1. - lambda) * mu;
                let depth_scale = wa.max(wb);
                let wa = wa / depth_scale;
                let wb = wb / depth_scale;
                // Virtual projected offsets are pa*wa² and pb*wb². Their
                // inverse projection uses opposite endpoint depths. This is
                // equivalent to minimizing |Hxy-cursor*W|² / W(1-t)².
                let qa = na.map(|x| x * wa * wa);
                let qb = nb.map(|x| x * wb * wb);
                let qscale = qa.into_iter().chain(qb).map(Real::abs).fold(0., Real::max);
                if qscale == 0. || wa == 0. || wb == 0. {
                    return None;
                }
                let qa = qa.map(|x| x / qscale);
                let qb = qb.map(|x| x / qscale);
                let d = [qb[0] - qa[0], qb[1] - qa[1]];
                let squared = d[0] * d[0] + d[1] * d[1];
                if squared == 0. {
                    return Some(a);
                }
                let from_a = -(qa[0] * d[0] + qa[1] * d[1]);
                let from_b = qb[0] * d[0] + qb[1] * d[1];
                if from_a <= 0. || from_b <= 0. {
                    return Some(if from_a <= 0. { a } else { b });
                }
                let first = from_a * wb;
                let second = from_b * wa;
                return if first <= second {
                    interpolate(a, b, first / (first + second))
                } else {
                    interpolate(b, a, second / (first + second))
                };
            }
            if probe == 0 && mu < 0.25 && orientation == 0 {
                break;
            }
            lambda *= 0.5;
            if lambda == 0. {
                return None;
            }
        }
        segment = Segment {
            a: segment.b,
            b: segment.a,
            pa: segment.pb,
            pb: segment.pa,
        };
    }
    None
}

/// Visible straight segments stay straight under the projection contract.
pub(super) fn segment_distance(a: [Real; 2], b: [Real; 2]) -> Option<Real> {
    let [x, y] = segment_offset(a, b);
    let distance = x.hypot(y);
    distance.is_finite().then_some(distance)
}

fn segment_offset(a: [Real; 2], b: [Real; 2]) -> [Real; 2] {
    // Normalize before differences/dots to avoid range loss in squares.
    let scale = a.into_iter().chain(b).map(Real::abs).fold(0., Real::max);
    if scale == 0. {
        return [0.; 2];
    }
    let normalized_a = a.map(|v| v / scale);
    let normalized_b = b.map(|v| v / scale);
    let d = [
        normalized_b[0] - normalized_a[0],
        normalized_b[1] - normalized_a[1],
    ];
    let squared = d[0] * d[0] + d[1] * d[1];
    let (a, b, t) = if squared == 0. {
        (a, b, 0.)
    } else {
        let from_a = -(normalized_a[0] * d[0] + normalized_a[1] * d[1]) / squared;
        let from_b = (normalized_b[0] * d[0] + normalized_b[1] * d[1]) / squared;
        // Use the smaller endpoint fraction, independently computed. This also
        // avoids two endpoint norms merely to choose an interpolation direction.
        if from_a <= from_b {
            (a, b, from_a.clamp(0., 1.))
        } else {
            (b, a, from_b.clamp(0., 1.))
        }
    };
    // Interpolate the original coordinates so a small perpendicular offset is
    // not lost when another axis has a vastly larger magnitude.
    std::array::from_fn(|i| (1. - t) * a[i] + t * b[i])
}

pub(super) fn interpolate(a: Point3, b: Point3, t: Real) -> Option<Point3> {
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
        // Compute distances from each endpoint independently: 1-s may round
        // to zero while the closest point is still far from that endpoint.
        let s = -(na[0] * d[0] + na[1] * d[1]) / squared;
        let complement = (nb[0] * d[0] + nb[1] * d[1]) / squared;
        if s <= 0. || complement <= 0. {
            return Some(if s <= 0. { a } else { b });
        }
        if metric.is_affine() {
            return if s <= complement {
                interpolate(a, b, s)
            } else {
                interpolate(b, a, complement)
            };
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
                let factor = (complement / s) * (mu / (1. - mu)) * (1. - lambda);
                return if lambda <= factor {
                    interpolate(a, b, lambda / (factor + lambda))
                } else {
                    interpolate(b, a, factor / (factor + lambda))
                };
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
