//! Query-local sampled diagnostics, never complete surface distance bounds.
use super::evaluate::SurfaceQuery;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePointPairWitness {
    pub source_parameters: [Real; 2],
    pub target_parameters: [Real; 2],
    pub source_point: Point3,
    pub target_point: Point3,
    pub distance: Real,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceDeviationSampleSummary {
    pub sample_count: usize,
    pub refinement_failures: usize,
    pub maximum: SurfacePointPairWitness,
}

/// Sampled point-pair witnesses. Local refinement need not find global nearest
/// points. No summary bounds the complete images or any unsampled points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceDeviationSamples {
    pub divisions: [usize; 2],
    pub same_parameters: SurfaceDeviationSampleSummary,
    pub first_to_second: SurfaceDeviationSampleSummary,
    pub second_to_first: SurfaceDeviationSampleSummary,
}

#[derive(Default)]
struct Accumulator {
    maximum: Option<SurfacePointPairWitness>,
    count: usize,
    failures: usize,
}
impl Accumulator {
    fn observe(
        &mut self,
        a: Point3,
        b: Point3,
        uv: [Real; 2],
        st: [Real; 2],
        failed: bool,
    ) -> Result<(), GeometryError> {
        let distance = a.distance_to(b)?;
        self.count += 1;
        self.failures += usize::from(failed);
        if self.maximum.is_none_or(|old| distance > old.distance) {
            self.maximum = Some(SurfacePointPairWitness {
                source_parameters: uv,
                target_parameters: st,
                source_point: a,
                target_point: b,
                distance,
            });
        }
        Ok(())
    }
    fn finish(self) -> SurfaceDeviationSampleSummary {
        SurfaceDeviationSampleSummary {
            sample_count: self.count,
            refinement_failures: self.failures,
            maximum: self
                .maximum
                .expect("positive division counts provide grid witnesses"),
        }
    }
}

fn local_witness(
    query: &mut SurfaceQuery<'_>,
    point: Point3,
    seed: [Real; 2],
    seed_point: Point3,
    tolerance: Tolerance,
) -> (Point3, [Real; 2], bool) {
    if point == seed_point {
        return (seed_point, seed, false);
    }
    let u = query.surface.domain_u();
    let v = query.surface.domain_v();
    match query
        .refine_closest_parameters(
            point,
            seed[0],
            seed[1],
            [*u.start(), *u.end()],
            [*v.start(), *v.end()],
            tolerance,
        )
        .and_then(|(s, t)| {
            let p = query.evaluate(s, t)?;
            Ok((p, [s, t], point.distance_to(p)?))
        }) {
        Ok((p, uv, d)) if point.distance_to(seed_point).is_ok_and(|old| d < old) => (p, uv, false),
        Ok(_) => (seed_point, seed, false),
        Err(_) => (seed_point, seed, true),
    }
}

impl NurbsSurface {
    /// Compare regular normalized-U/V grids and local point-pair witnesses in
    /// both directions, preserving each original parameter domain. Refinement
    /// starts at corresponding normalized parameters; it is not a global
    /// closest-point search. Failed refinements retain valid seed witnesses
    /// and are counted. Even zero sampled distance can miss an interior spike.
    ///
    /// Inputs remain unchanged. Requires 1..=64 divisions per axis, degrees
    /// <=16 and <=4096 controls per surface. Two query-owned evaluation caches
    /// are reused throughout; no global state or source copies are introduced.
    pub fn deviation_samples(
        &self,
        other: &Self,
        divisions: [usize; 2],
        tolerance: Tolerance,
    ) -> Result<SurfaceDeviationSamples, GeometryError> {
        if divisions.iter().any(|n| !(1..=64).contains(n)) {
            return Err(GeometryError::UnsupportedSurfaceApproximation {
                context: "surface deviation sampling requires 1..=64 divisions per axis",
            });
        }
        if [self, other]
            .iter()
            .any(|s| s.degree_u() > 16 || s.degree_v() > 16 || s.control_points().len() > 4096)
        {
            return Err(GeometryError::SurfaceApproximationLimit);
        }
        let mut first = SurfaceQuery::new(self);
        let mut second = SurfaceQuery::new(other);
        let mut same = Accumulator::default();
        let mut forward = Accumulator::default();
        let mut reverse = Accumulator::default();
        for j in 0..=divisions[1] {
            for i in 0..=divisions[0] {
                let f = [
                    i as Real / divisions[0] as Real,
                    j as Real / divisions[1] as Real,
                ];
                let a = [self.parameter_at_u(f[0])?, self.parameter_at_v(f[1])?];
                let b = [other.parameter_at_u(f[0])?, other.parameter_at_v(f[1])?];
                let pa = first.evaluate(a[0], a[1])?;
                let pb = second.evaluate(b[0], b[1])?;
                same.observe(pa, pb, a, b, false)?;
                let (p, uv, failed) = local_witness(&mut second, pa, b, pb, tolerance);
                forward.observe(pa, p, a, uv, failed)?;
                let (p, uv, failed) = local_witness(&mut first, pb, a, pa, tolerance);
                reverse.observe(pb, p, b, uv, failed)?;
            }
        }
        Ok(SurfaceDeviationSamples {
            divisions,
            same_parameters: same.finish(),
            first_to_second: forward.finish(),
            second_to_first: reverse.finish(),
        })
    }
}

#[cfg(test)]
mod tests;
