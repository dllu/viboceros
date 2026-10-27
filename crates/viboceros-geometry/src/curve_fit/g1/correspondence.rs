//! Numerical correspondence proposals accepted only by complete image bounds.
use crate::{GeometryError, NurbsCurve, Real, Tolerance};
use std::collections::{BTreeMap, BinaryHeap};

const MAX_INTERVALS: usize = 256;
mod anchors;
mod hermite;
mod linear;

pub(crate) fn within_deviation(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    limit: Real,
) -> Result<bool, GeometryError> {
    certify(source, candidate, limit, None)
}

/// Seed known source/target span boundaries in normalized parameter fractions.
/// They guide proposals only; the full exact image certificate still decides.
pub(crate) fn within_deviation_at_pairs(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    limit: Real,
    pairs: &[[Real; 2]],
) -> Result<bool, GeometryError> {
    certify(source, candidate, limit, Some(pairs))
}

fn certify(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    limit: Real,
    pairs: Option<&[[Real; 2]]>,
) -> Result<bool, GeometryError> {
    if source
        .parameter_curve_deviation_bound(candidate, limit)?
        .is_some()
    {
        return Ok(true);
    }
    if linear::certify(source, candidate, limit) {
        return Ok(true);
    }
    match source.monotone_projected_curve_deviation_bound(candidate, limit) {
        Ok(Some(_)) => return Ok(true),
        Ok(None) | Err(GeometryError::CurveApproximationLimit) => {}
        Err(error) => return Err(error),
    }
    let sampler = source.parameter_sampler()?;
    let search = candidate.prepare_closest_parameter_search()?;
    let numerical = Tolerance::try_new((limit * 1e-6).max(Real::from_bits(1)), 1e-12, 1e-10)?;
    let (mut stations, derivatives) = if let Some(prepared) = pairs
        .map(|pairs| anchors::prepare(source, candidate, pairs))
        .transpose()?
        .flatten()
    {
        prepared
    } else {
        (Stations::new(source)?, BTreeMap::new())
    };
    let mut attempted_size = 0;
    for intervals in [8, 16, 32, 64, 128, MAX_INTERVALS] {
        if !stations.refine(intervals + 1) {
            return Ok(false);
        }
        if stations.parameters.len() == attempted_size {
            continue;
        }
        attempted_size = stations.parameters.len();
        let mut pairs = Vec::with_capacity(attempted_size);
        for (&bits, cached) in &mut stations.parameters {
            let fraction = Real::from_bits(bits);
            let parameter = if let Some(parameter) = *cached {
                parameter
            } else {
                let point = sampler.evaluate(fraction)?;
                let parameter = search.closest_parameter(point, numerical)?;
                // This only declines a numerical proposal. Accepted fits still
                // require a complete exact certificate across every interval.
                if point.distance_to(candidate.evaluate(parameter)?)? > limit {
                    return Ok(false);
                }
                // Searches return the candidate's native parameter, while
                // cached anchors and both mapped certificates use fractions.
                // Keep the stored curve and its complete proof unchanged.
                let domain = candidate.domain();
                let parameter =
                    crate::nurbs::interval_fraction(parameter, *domain.start(), *domain.end())?;
                *cached = Some(parameter);
                parameter
            };
            if pairs.last().is_some_and(|p: &[Real; 2]| p[1] >= parameter) {
                return Ok(false);
            }
            pairs.push([fraction, parameter]);
        }
        if source
            .mapped_curve_deviation_bound(candidate, &pairs, limit)?
            .is_some()
        {
            return Ok(true);
        }
        if hermite::certify_with_derivatives(source, candidate, &pairs, limit, &derivatives)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Natural boundaries prevent a correspondence interval from spanning a speed
/// change. Bisect the largest remaining gaps without increasing the existing
/// station/query budget. Unit fractions have numerically ordered bit patterns.
struct Stations {
    parameters: BTreeMap<u64, Option<Real>>,
    gaps: BinaryHeap<(u64, u64, u64)>,
}

impl Stations {
    fn new(source: &NurbsCurve) -> Result<Self, GeometryError> {
        let domain = source.domain();
        let (start, end) = (*domain.start(), *domain.end());
        let mut parameters = BTreeMap::new();
        let mut gaps = BinaryHeap::new();
        let mut previous = None;
        for group in
            source.knots()[source.degree()..=source.control_points().len()].chunk_by(|a, b| a == b)
        {
            let knot = group[0];
            let fraction = if knot == start {
                0.
            } else if knot == end {
                1.
            } else {
                crate::nurbs::interval_fraction(knot, start, end)?
            };
            if let Some(before) = previous {
                if fraction <= before {
                    return Ok(Self::uniform());
                }
                gaps.push((
                    (fraction - before).to_bits(),
                    before.to_bits(),
                    fraction.to_bits(),
                ));
            }
            parameters.insert(
                fraction.to_bits(),
                (fraction == 0. || fraction == 1.).then_some(fraction),
            );
            if parameters.len() > MAX_INTERVALS + 1 {
                return Ok(Self::uniform());
            }
            previous = Some(fraction);
        }
        Ok(Self { parameters, gaps })
    }

    // Preserve the former proposal when all natural knots cannot fit in the
    // fractional chart or station budget. The complete bound still decides.
    fn uniform() -> Self {
        Self {
            parameters: BTreeMap::from([(0_f64.to_bits(), Some(0.)), (1_f64.to_bits(), Some(1.))]),
            gaps: BinaryHeap::from([(1_f64.to_bits(), 0_f64.to_bits(), 1_f64.to_bits())]),
        }
    }

    fn refine(&mut self, size: usize) -> bool {
        while self.parameters.len() < size {
            let Some((_, first, last)) = self.gaps.pop() else {
                return false;
            };
            let (a, b) = (Real::from_bits(first), Real::from_bits(last));
            let middle = a.midpoint(b);
            if middle <= a || middle >= b {
                return false;
            }
            self.parameters.insert(middle.to_bits(), None);
            self.gaps
                .push(((middle - a).to_bits(), first, middle.to_bits()));
            self.gaps
                .push(((b - middle).to_bits(), middle.to_bits(), last));
        }
        true
    }
}
