//! Bounded tangent contacts from curve derivatives, without tessellation.
use crate::curve_contact::{Domain, project, seeds};
use crate::{CurveRef, GeometryError, Point3, Real, Tolerance, UnitVector3, Vector3};

fn tangent_jet(
    c: CurveRef<'_>,
    t: Real,
    normal: Option<UnitVector3>,
) -> Result<(Point3, Vector3, Vector3, Vector3), GeometryError> {
    let (p, d, dd) = c.evaluate_with_second_derivative(t)?;
    let d = project(d, normal)?;
    let dd = project(dd, normal)?;
    let speed = d.length()?;
    let tangent = d.normalized_nonzero()?.as_vector();
    let parallel = tangent.scaled(tangent.dot(dd)?)?;
    let derivative = Vector3::try_from(std::array::from_fn(|i| {
        dd.to_array()[i] - parallel.to_array()[i]
    }))?
    .scaled(1. / speed)?;
    Ok((p, d, tangent, derivative))
}

fn root(
    c: CurveRef<'_>,
    domain: Domain,
    target: Point3,
    seed: Real,
    normal: Option<UnitVector3>,
) -> Option<Real> {
    let mut t = seed;
    for _ in 0..64 {
        let (p, _, tangent, derivative) = tangent_jet(c, t, normal).ok()?;
        let r = project(p.vector_to(target).ok()?, normal).ok()?;
        let f = r.cross(tangent).ok()?;
        if f.length().ok()? <= 128. * Real::EPSILON * r.length().ok()?.max(1.) {
            return Some(t);
        }
        let df = r.cross(derivative).ok()?;
        let squared = df.dot(df).ok()?;
        if squared == 0. {
            return None;
        }
        let step = f.dot(df).ok()? / squared;
        if !step.is_finite() {
            return None;
        }
        let next = domain.bound(t - step);
        if next == t {
            break;
        }
        t = next;
    }
    None
}

impl CurveRef<'_> {
    /// Find a tangent whose contact lies in this curve's domain. A point
    /// outside a spatial curve's tangent developable has no exact solution.
    /// `seed` determines the preferred branch when several contacts exist.
    pub fn tangent_parameter_near(
        self,
        target: Point3,
        seed: Real,
        normal: Option<UnitVector3>,
        tolerance: Tolerance,
    ) -> Result<Option<Real>, GeometryError> {
        self.evaluate(seed)?;
        let linear = match self {
            Self::Line(_) => true,
            Self::NurbsCurve(c) => c.is_linear(tolerance)?,
            _ => false,
        };
        if linear {
            let t = self.closest_parameter(target, tolerance)?;
            let (p, _, tangent, _) = tangent_jet(self, t, normal)?;
            return Ok((project(p.vector_to(target)?, normal)?
                .cross(tangent)?
                .length()?
                <= tolerance.absolute())
            .then_some(t));
        }

        let domain = Domain::new(self)?;
        let seeds = seeds(self, seed)?;
        let distance = |t: Real| (t - seed).abs();
        let mut best: Option<Real> = None;
        for t in seeds
            .into_iter()
            .filter_map(|s| root(self, domain, target, s, normal))
        {
            let (p, _, tangent, _) = tangent_jet(self, t, normal)?;
            if project(p.vector_to(target)?, normal)?
                .cross(tangent)?
                .length()?
                > tolerance.absolute()
            {
                continue;
            }
            if best.is_none_or(|old| distance(t) < distance(old)) {
                best = Some(t);
            }
        }
        Ok(best)
    }
}

/// Refine a pair of picked contacts to a common tangent. Both source domains
/// remain bounded; skew lines and separated spatial tangent lines fail.
pub fn common_tangent_near(
    first: CurveRef<'_>,
    first_seed: Real,
    second: CurveRef<'_>,
    second_seed: Real,
    tolerance: Tolerance,
) -> Result<Option<[Point3; 2]>, GeometryError> {
    let (a_domain, b_domain) = (Domain::new(first)?, Domain::new(second)?);
    let (mut u, mut v) = (first_seed, second_seed);
    for _ in 0..64 {
        let (a, da, ta, dta) = tangent_jet(first, u, None)?;
        let (b, db, tb, dtb) = tangent_jet(second, v, None)?;
        let r = b.vector_to(a)?;
        let (fa, fb) = (r.cross(ta)?, r.cross(tb)?);
        let residual = fa.length()?.max(fb.length()?);
        if residual <= 128. * Real::EPSILON * r.length()?.max(1.) {
            return Ok((a.distance_to(b)? > tolerance.absolute()).then_some([a, b]));
        }
        let j00 = r.cross(dta)?;
        let j01 = db.cross(ta)?.scaled(-1.)?;
        let j10 = da.cross(tb)?;
        let j11 = r.cross(dtb)?;
        let h00 = j00.dot(j00)? + j10.dot(j10)?;
        let h01 = j00.dot(j01)? + j10.dot(j11)?;
        let h11 = j01.dot(j01)? + j11.dot(j11)?;
        let g0 = j00.dot(fa)? + j10.dot(fb)?;
        let g1 = j01.dot(fa)? + j11.dot(fb)?;
        let Some(step) = nalgebra::Matrix2::new(h00, h01, h01, h11)
            .lu()
            .solve(&nalgebra::Vector2::new(-g0, -g1))
            .filter(|s| s.iter().all(|x| x.is_finite()))
        else {
            return Ok(None);
        };
        let (nu, nv) = (a_domain.bound(u + step.x), b_domain.bound(v + step.y));
        if (nu, nv) == (u, v) {
            break;
        }
        (u, v) = (nu, nv);
    }
    Ok(None)
}
