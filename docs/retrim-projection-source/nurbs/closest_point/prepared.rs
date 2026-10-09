//! Independent repeated-target searches with bounded coefficient preparation.
use super::*;

/// Curve-owned preparation for repeated independent targets. Seed points are
/// evaluated in original coordinates; translated controls only propose parameters.
pub(crate) struct PreparedCurveClosest<'a> {
    source: &'a NurbsCurve,
    local: Option<NurbsCurve>,
    offset: Vector3,
    seeds: Vec<(Real, Point3)>,
    domain: [Real; 2],
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
        })
    }
    pub(crate) fn closest_parameter(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let mut query = CurveQuery::retaining_spans(self.source);
        let translated = target.translated(self.offset);
        let (refinement, refinement_target) = match (&self.local, translated) {
            (Some(local), Ok(point)) => (local, point),
            _ => (self.source, target),
        };
        let mut refinement = CurveQuery::retaining_spans(refinement);
        closest_from_seeds(
            &mut query,
            &mut refinement,
            self.seeds.iter().copied(),
            target,
            refinement_target,
            self.domain,
            tolerance,
        )
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
