//! Local branches of bounded curve normals, including common perpendiculars.
use crate::{CurveRef, GeometryError, Point3, Real, Tolerance, UnitVector3};

use crate::curve_contact::{Domain, project, seeds};
fn root(
    c: CurveRef<'_>,
    domain: Domain,
    target: Point3,
    seed: Real,
    normal: Option<UnitVector3>,
) -> Option<Real> {
    let mut t = seed;
    for _ in 0..48 {
        let (p, d, dd) = c.evaluate_with_second_derivative(t).ok()?;
        let r = project(target.vector_to(p).ok()?, normal).ok()?;
        let d = project(d, normal).ok()?;
        let dd = project(dd, normal).ok()?;
        let speed = d.length().ok()?;
        if speed == 0. {
            return normal.is_some().then_some(t);
        }
        let f = r.dot(d).ok()?;
        if (f / speed).abs() <= 64. * Real::EPSILON * r.length().ok()?.max(1.) {
            return Some(t);
        }
        let df = d.dot(d).ok()? + r.dot(dd).ok()?;
        if df == 0. || !df.is_finite() {
            return None;
        }
        let step = f / df;
        if !step.is_finite() {
            return None;
        }
        let next = domain.bound(t - step);
        if next == t {
            break;
        }
        t = next;
    }
    let (p, d) = c.evaluate_with_derivative(t).ok()?;
    let d = project(d, normal).ok()?.normalized_nonzero().ok()?;
    let residual = project(target.vector_to(p).ok()?, normal).ok()?;
    (residual.dot(d.as_vector()).ok()?.abs() <= 1e-10 * residual.length().ok()?.max(1.))
        .then_some(t)
}
impl CurveRef<'_> {
    /// Choose a perpendicular contact on the branch nearest a picked station.
    /// Both minima and maxima matter: picking the far side of a closed curve
    /// deliberately chooses its farther normal. If no stationary point lies in
    /// an open domain, return the closest bounded endpoint for normal-plane use.
    /// An optional normal removes one coordinate for planar cursor tracking.
    pub fn perpendicular_parameter_near(
        self,
        target: Point3,
        seed: Real,
        normal: Option<UnitVector3>,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        self.evaluate(seed)?;
        let seeds = seeds(self, seed)?;
        // Closure tests can inspect the control net. Cache them once per
        // query rather than repeating that work at every Newton step.
        let domain = Domain::new(self)?;
        let (a, b, closed) = (domain.a, domain.b, domain.closed);
        let distance = |t: Real| {
            let d = (t - seed).abs();
            if closed { d.min((b - a - d).abs()) } else { d }
        };
        let mut best: Option<Real> = None;
        for t in seeds
            .into_iter()
            .filter_map(|s| root(self, domain, target, s, normal))
        {
            if best.is_none_or(|old| distance(t) < distance(old)) {
                best = Some(t);
            }
        }
        if let Some(t) = best {
            return Ok(t);
        }
        if normal.is_none() {
            return self.closest_parameter(target, tolerance);
        }
        // Bounded planar fallback compares only projected endpoint distances.
        let da = project(target.vector_to(self.evaluate(a)?)?, normal)?.length()?;
        let db = project(target.vector_to(self.evaluate(b)?)?, normal)?.length()?;
        Ok(if da <= db { a } else { b })
    }
}
/// Refine the picked pair rather than taking a global minimum: far-side curve
/// normals are intentional. A bounded nonstationary pair is not perpendicular.
pub fn common_perpendicular_near(
    first: CurveRef<'_>,
    first_seed: Real,
    second: CurveRef<'_>,
    second_seed: Real,
    tolerance: Tolerance,
) -> Result<Option<[Point3; 2]>, GeometryError> {
    let (mut u, mut v) = (first_seed, second_seed);
    let first_domain = Domain::new(first)?;
    let second_domain = Domain::new(second)?;
    for _ in 0..64 {
        let (a, du, ddu) = first.evaluate_with_second_derivative(u)?;
        let (b, dv, ddv) = second.evaluate_with_second_derivative(v)?;
        let r = b.vector_to(a)?;
        let (f, g) = (r.dot(du)?, -r.dot(dv)?);
        let residual = (f.abs() / du.length()?).max(g.abs() / dv.length()?);
        if residual <= 64. * Real::EPSILON * r.length()?.max(1.) {
            return Ok(Some([a, b]));
        }
        let h00 = du.dot(du)? + r.dot(ddu)?;
        let h11 = dv.dot(dv)? - r.dot(ddv)?;
        let h01 = -du.dot(dv)?;
        let matrix = nalgebra::Matrix2::new(h00, h01, h01, h11);
        let next = matrix.lu().solve(&nalgebra::Vector2::new(-f, -g));
        let (nu, nv) = if let Some(step) = next.filter(|s| s.iter().all(|v| v.is_finite())) {
            (
                first_domain.bound(u + step.x),
                second_domain.bound(v + step.y),
            )
        } else {
            // Parallel lines have a family of solutions. Retain the second
            // picked station and project the first curve to it.
            (
                first.perpendicular_parameter_near(b, u, None, tolerance)?,
                v,
            )
        };
        if (nu, nv) == (u, v) {
            break;
        }
        (u, v) = (nu, nv);
    }
    Ok(None)
}
