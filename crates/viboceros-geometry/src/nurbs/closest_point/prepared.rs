//! Independent repeated-target searches with bounded coefficient preparation.
use super::super::evaluate::CurveQueryState;
use super::*;
use std::cell::RefCell;

/// Curve-owned preparation for repeated independent targets. Seed points are
/// evaluated in original coordinates; translated controls only propose parameters.
pub(crate) struct PreparedCurveClosest<'a> {
    source: &'a NurbsCurve,
    local: Option<NurbsCurve>,
    offset: Vector3,
    seeds: Vec<(Real, Point3)>,
    domain: [Real; 2],
    queries: RefCell<[CurveQueryState; 3]>,
}
impl<'a> PreparedCurveClosest<'a> {
    pub(super) fn new(source: &'a NurbsCurve) -> Result<Self, GeometryError> {
        let origin = source.control_points[0].point;
        let offset = Vector3::try_new(-origin.x(), -origin.y(), -origin.z())?;
        let local = (origin.to_array() != [0.; 3])
            .then(|| source.transformed(AffineTransform3::from_translation(offset)))
            .and_then(Result::ok);
        let domain = [*source.domain().start(), *source.domain().end()];
        let mut query = CurveQuery::new(source);
        let seeds = curve_closest_parameter_seeds(source.spans(), domain[0], domain[1])
            .into_iter()
            .filter_map(|parameter| {
                query
                    .evaluate(parameter)
                    .ok()
                    .map(|point| (parameter, point))
            })
            .collect();
        Ok(Self {
            source,
            local,
            offset,
            seeds,
            domain,
            queries: RefCell::new(std::array::from_fn(|_| CurveQueryState::retaining_spans())),
        })
    }
    pub(crate) fn closest_parameter(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let mut states = self.queries.borrow_mut();
        let mut query = CurveQuery::with_state(self.source, std::mem::take(&mut states[0]));
        let translated = target.translated(self.offset);
        let (refinement, refinement_target, index) = match (&self.local, translated) {
            (Some(local), Ok(point)) => (local, point, 1),
            _ => (self.source, target, 2),
        };
        let mut refinement = CurveQuery::with_state(refinement, std::mem::take(&mut states[index]));
        let result = closest_from_seeds(
            &mut query,
            &mut refinement,
            self.seeds.iter().copied(),
            target,
            refinement_target,
            self.domain,
            tolerance,
        );
        states[0] = query.into_state();
        states[index] = refinement.into_state();
        result
    }
}

fn closest_from_seeds(
    query: &mut CurveQuery<'_>,
    refinement: &mut CurveQuery<'_>,
    seeds: impl Iterator<Item = (Real, Point3)>,
    target: Point3,
    refinement_target: Point3,
    domain: [Real; 2],
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let mut candidates = Vec::new();
    for (parameter, point) in seeds {
        let candidate = Candidate {
            parameter,
            distance: PointDistance::new(target, point),
        };
        if parameter == domain[0] && point == target {
            return Ok(parameter);
        }
        candidates.push(candidate);
    }
    // Every original seed is retained, including long and stationary spans.
    let mut best = candidates
        .iter()
        .min_by(|a, b| a.compare(b, target))
        .copied()
        .ok_or(GeometryError::Degenerate {
            context: "NURBS curve closest-point search",
        })?;
    for seed in candidates {
        if let Ok(parameter) = refinement.refine_closest_parameter_only(
            refinement_target,
            seed.parameter,
            domain,
            tolerance,
        ) && let Ok(candidate) = Candidate::new(query, target, parameter)
            && candidate.compare(&best, target).is_lt()
        {
            best = candidate;
        }
    }
    Ok(best.parameter)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_translation_fallback_never_crosses_coefficient_frames() {
        let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let source = NurbsCurve::try_new(
            1,
            vec![p(Real::MAX, 0., 0.), p(Real::MAX, 1., 0.)],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let prepared = PreparedCurveClosest::new(&source).unwrap();
        for _ in 0..3 {
            for target in [
                p(Real::MAX, 0.37, 0.),
                p(-Real::MAX, 0.37, 0.),
                p(Real::MAX, 0.63, 0.),
            ] {
                let actual = prepared
                    .closest_parameter(target, Tolerance::DEFAULT)
                    .unwrap();
                assert_eq!(
                    actual,
                    source
                        .closest_parameter(target, Tolerance::DEFAULT)
                        .unwrap()
                );
                assert!((actual - target.y()).abs() < 1e-12);
            }
        }
        // The original comparison and both refinement frames received visits.
        let states = prepared.queries.borrow();
        assert!(states.iter().all(|state| state.has_prepared_span()));
    }
}
