//! Locate the curvature landmarks used by the viewport's curve getter.
use viboceros_geometry::{CurveRef, NurbsCurve, Point3, UnitVector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvatureFeature {
    pub parameter: f64,
    pub point: Point3,
    pub inflection: bool,
}

#[derive(Clone, Copy)]
struct Station {
    parameter: f64,
    magnitude: f64,
    direction: Option<UnitVector3>,
}

impl Station {
    fn sample(curve: CurveRef<'_>, parameter: f64) -> Option<Self> {
        let v = curve.curvature_vector(parameter).ok()?;
        Some(Self {
            parameter,
            magnitude: v.length().ok()?,
            direction: v.normalized_nonzero().ok(),
        })
    }

    fn opposite(self, next: Self) -> bool {
        self.direction
            .zip(next.direction)
            .and_then(|(a, b)| a.as_vector().dot(b.as_vector()).ok())
            .is_some_and(|dot| dot < 0.)
    }
}

/// The exact rational representation provides a subdivision of the native
/// parameter range. Curvature itself is evaluated on the original curve.
pub fn curve_curvature_features(
    curve: CurveRef<'_>,
    display: &NurbsCurve,
) -> Vec<CurvatureFeature> {
    let feature = |parameter, inflection| {
        Some(CurvatureFeature {
            parameter,
            point: curve.evaluate(parameter).ok()?,
            inflection,
        })
    };
    match curve {
        CurveRef::Line(_) | CurveRef::Polyline(_) | CurveRef::Circle(_) | CurveRef::Arc(_) => {
            return Vec::new();
        }
        CurveRef::Ellipse(ellipse) => {
            let stations = match ellipse.radius_x().total_cmp(&ellipse.radius_y()) {
                std::cmp::Ordering::Greater => [0., 0.5],
                std::cmp::Ordering::Less => [0.25, 0.75],
                std::cmp::Ordering::Equal => return Vec::new(),
            };
            return stations
                .into_iter()
                .filter_map(|station| feature(curve.parameter_at(station).ok()?, false))
                .collect();
        }
        _ => {}
    }

    const STEPS: usize = 12;
    const MAX_SPANS: usize = 4096;
    if display.spans().take(MAX_SPANS + 1).count() > MAX_SPANS {
        return Vec::new();
    }
    let mut stations = Vec::new();
    for (start, end) in display.spans() {
        stations.extend((0..STEPS).map(|i| {
            let fraction = i as f64 / STEPS as f64;
            Station::sample(curve, start * (1. - fraction) + end * fraction)
        }));
    }
    stations.push(Station::sample(curve, *curve.domain().end()));

    let mut features = Vec::new();
    let mut add = |t, inflection| {
        if let Some(value) = feature(t, inflection) {
            features.push(value);
        }
    };
    // An isolated zero is an inflection only when the direction switches.
    // In 3-D, a rotating nonzero vector can also reverse between two samples.
    // The bounded search therefore has to recover a near-zero magnitude.
    let measure_inflection = |a: Station, b: Station| {
        if !a.opposite(b) {
            return None;
        }
        let min = extremum(curve, a, b, false)?;
        (min.magnitude <= 1e-8 * a.magnitude.max(b.magnitude)).then_some(min.parameter)
    };
    for pair in stations.windows(2) {
        if let [Some(a), Some(b)] = pair
            && let Some(t) = measure_inflection(*a, *b)
        {
            add(t, true);
        }
    }
    for triplet in stations.windows(3) {
        let [Some(a), Some(b), Some(c)] = triplet else {
            continue;
        };
        if b.direction.is_none()
            && let Some(t) = measure_inflection(*a, *c)
        {
            add(t, true);
        }
        if strict_maximum(*a, *b, *c)
            && let Some(maximum) = extremum(curve, *a, *c, true)
        {
            add(maximum.parameter, false);
        }
    }
    if curve.is_closed().unwrap_or(false)
        && let ([Some(first), Some(second), ..], [.., Some(before_end), Some(_)]) =
            (stations.as_slice(), stations.as_slice())
        && strict_maximum(*before_end, *first, *second)
    {
        add(first.parameter, false);
    }
    features.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    features.dedup_by(|a, b| {
        a.inflection == b.inflection
            && (a.parameter - b.parameter).abs()
                <= 32. * f64::EPSILON * a.parameter.abs().max(b.parameter.abs())
    });
    features
}

fn strict_maximum(a: Station, b: Station, c: Station) -> bool {
    let lower = a.magnitude.max(c.magnitude);
    b.magnitude - lower > 1e-10 * b.magnitude
}

fn extremum(
    curve: CurveRef<'_>,
    mut a: Station,
    mut b: Station,
    maximize: bool,
) -> Option<Station> {
    let rank = |station: Station| {
        if maximize {
            -station.magnitude
        } else {
            station.magnitude
        }
    };
    let mut best = if rank(a) < rank(b) { a } else { b };
    for _ in 0..48 {
        let left = a.parameter * (2. / 3.) + b.parameter * (1. / 3.);
        let right = a.parameter * (1. / 3.) + b.parameter * (2. / 3.);
        if !(a.parameter < left && left < right && right < b.parameter) {
            break;
        }
        let l = Station::sample(curve, left)?;
        let r = Station::sample(curve, right)?;
        for station in [l, r] {
            if rank(station) < rank(best) {
                best = station;
            }
        }
        if rank(l) <= rank(r) {
            b = r;
        } else {
            a = l;
        }
    }
    Some(best)
}
