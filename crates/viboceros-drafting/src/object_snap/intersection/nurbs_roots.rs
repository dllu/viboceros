//! Isolate scalar zeros on one exact fractional NURBS knot span.
use viboceros_geometry::Real;

pub(super) fn isolate(
    score: &impl Fn(Real) -> Option<Real>,
    derivative: &impl Fn(Real) -> Option<Real>,
    stations: usize,
    tolerance: Real,
) -> Vec<Real> {
    let values: Vec<Option<Real>> = (0..=stations)
        .map(|i| score(i as Real / stations as Real))
        .collect();
    if values.iter().flatten().all(|v| v.abs() <= tolerance) {
        return Vec::new(); // No isolated target along a projected overlap.
    }
    let mut roots = Vec::new();
    for i in 0..stations {
        let a = i as Real / stations as Real;
        let b = (i + 1) as Real / stations as Real;
        if values[i].is_some_and(|v| v.abs() <= tolerance) {
            roots.push(a);
        }
        if let (Some(fa), Some(fb)) = (values[i], values[i + 1])
            && fa.signum() != fb.signum()
        {
            roots.push(bisect(score, a, b, fa));
        }
        if let (Some(da), Some(db)) = (derivative(a), derivative(b))
            && da.signum() != db.signum()
        {
            let stationary = bisect(derivative, a, b, da);
            if let Some(value) = score(stationary) {
                if value.abs() <= tolerance {
                    roots.push(stationary);
                } else {
                    // Two close crossings can straddle an extremum inside
                    // the same sampling interval.
                    if let Some(fa) = values[i]
                        && fa.signum() != value.signum()
                    {
                        roots.push(bisect(score, a, stationary, fa));
                    }
                    if let Some(fb) = values[i + 1]
                        && value.signum() != fb.signum()
                    {
                        roots.push(bisect(score, stationary, b, value));
                    }
                }
            }
        }
    }
    if values[stations].is_some_and(|v| v.abs() <= tolerance) {
        roots.push(1.);
    }
    roots.sort_by(Real::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() <= 1e-10);
    roots
}

fn bisect(f: &impl Fn(Real) -> Option<Real>, mut a: Real, mut b: Real, mut fa: Real) -> Real {
    for _ in 0..72 {
        let middle = a * 0.5 + b * 0.5;
        if middle == a || middle == b {
            break;
        }
        let Some(fm) = f(middle) else {
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
