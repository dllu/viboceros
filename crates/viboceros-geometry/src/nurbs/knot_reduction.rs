//! Basis-removal proposals accepted only by a complete original-curve bound.
use super::*;

pub const MAX_KNOT_REDUCTION_GROUPS: usize = 256;

/// A reduced curve and the number of interior knots removed. A finite bound
/// covers the complete original curve in normalized parameter correspondence.
/// `None` identifies an explicitly unrestricted operation.
#[derive(Clone, Debug, PartialEq)]
pub struct KnotReduction {
    pub curve: NurbsCurve,
    pub removed: usize,
    pub maximum_deviation_bound: Option<Real>,
}

impl NurbsCurve {
    /// Reduce an exact interior knot to at most `target_multiplicity`, accepting
    /// only a complete positional bound against the original stored curve.
    /// Zero tolerance requires exact agreement; `None` means unproved or outside
    /// the limit. Endpoint and tangent preservation are not promised.
    pub fn try_reduce_knot_multiplicity_with_bound(
        &self,
        knot: Real,
        target_multiplicity: usize,
        limit: Real,
    ) -> Result<Option<KnotReduction>, GeometryError> {
        validate(self, Some(limit))?;
        let multiplicity = self
            .interior_knot_groups()
            .into_iter()
            .find_map(|(value, count)| (value == knot).then_some(count))
            .ok_or(GeometryError::NoRemovableKnot { parameter: knot })?;
        if target_multiplicity >= multiplicity {
            return Ok(Some(unchanged(self, Some(limit))));
        }
        attempt(
            self,
            self,
            knot,
            multiplicity - target_multiplicity,
            Some(limit),
            &mut portable_removal,
        )
        .map(|proposal| {
            proposal.map(|(curve, bound)| KnotReduction {
                curve,
                removed: multiplicity - target_multiplicity,
                maximum_deviation_bound: bound,
            })
        })
    }

    /// Collapse eligible multiple knots to simple knots, in descending order.
    /// The multiplicity range is exclusive; degree-one interior knots are also
    /// eligible when the minimum is <=1 and the maximum is >1, and disappear.
    /// Every accepted group is certified against `self`, so errors cannot add
    /// across groups. `Some(0.)` is strict; `None` explicitly permits any change.
    /// An unresolved certificate declines that group. Periodic sources and more
    /// than 256 eligible groups fail before editing. Certificate degree, bit
    /// and work limits are those of `parameter_curve_deviation_bound`.
    pub fn try_reduce_knots_in_range(
        &self,
        minimum: usize,
        maximum: usize,
        limit: Option<Real>,
    ) -> Result<KnotReduction, GeometryError> {
        self.try_reduce_knots_in_range_with_remover(minimum, maximum, limit, &mut portable_removal)
    }

    /// Retain range selection and the original-curve certificate while using a
    /// caller's basis editor. The returned degree and full knot vector must
    /// exactly match the supplied basis. A callback cannot bypass the bound.
    pub fn try_reduce_knots_in_range_with_remover<E: From<GeometryError>>(
        &self,
        minimum: usize,
        maximum: usize,
        limit: Option<Real>,
        remove: &mut impl FnMut(&NurbsCurve, &[Real]) -> Result<NurbsCurve, E>,
    ) -> Result<KnotReduction, E> {
        if minimum >= maximum {
            return Err(GeometryError::InvalidKnotReductionRange { minimum, maximum }.into());
        }
        validate(self, limit)?;
        let groups = self
            .interior_knot_groups()
            .into_iter()
            .filter(|&(_, count)| {
                count < maximum
                    && if self.degree == 1 {
                        minimum <= 1
                    } else {
                        count > minimum.max(1)
                    }
            })
            .collect::<Vec<_>>();
        if groups.len() > MAX_KNOT_REDUCTION_GROUPS {
            return Err(GeometryError::CurveApproximationLimit.into());
        }
        let mut result = unchanged(self, limit);
        for (knot, count) in groups.into_iter().rev() {
            let removed = if self.degree == 1 { count } else { count - 1 };
            if let Some((curve, bound)) =
                attempt(self, &result.curve, knot, removed, limit, remove)?
            {
                result.curve = curve;
                result.removed += removed;
                result.maximum_deviation_bound = bound;
            }
        }
        Ok(result)
    }
}

fn validate(source: &NurbsCurve, limit: Option<Real>) -> Result<(), GeometryError> {
    if limit.is_some_and(|value| !value.is_finite() || value < 0.) {
        return Err(GeometryError::InvalidTolerance);
    }
    if source.is_periodic() {
        return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" });
    }
    if limit.is_some()
        && (source.degree > 16
            || source.control_points.iter().any(|point| {
                point.weight() == 0.
                    || point.weight().is_sign_negative()
                        != source.control_points[0].weight().is_sign_negative()
            }))
    {
        return Err(GeometryError::UnsupportedCurveApproximation {
            context: "bounded knot reduction requires degree <=16 and nonzero common-sign weights",
        });
    }
    Ok(())
}

fn unchanged(source: &NurbsCurve, limit: Option<Real>) -> KnotReduction {
    KnotReduction {
        curve: source.clone(),
        removed: 0,
        maximum_deviation_bound: limit.map(|_| 0.),
    }
}

fn portable_removal(source: &NurbsCurve, knots: &[Real]) -> Result<NurbsCurve, GeometryError> {
    source.interpolate_homogeneous_in_basis(
        source.degree,
        knots.to_vec(),
        GeometryError::KnotRemovalSolveFailed,
    )
}

fn attempt<E: From<GeometryError>>(
    original: &NurbsCurve,
    current: &NurbsCurve,
    knot: Real,
    count: usize,
    limit: Option<Real>,
    remove: &mut impl FnMut(&NurbsCurve, &[Real]) -> Result<NurbsCurve, E>,
) -> Result<Option<(NurbsCurve, Option<Real>)>, E> {
    let clamped = current.clamped_to_active_domain()?;
    let knots = knot_group_removal_knots(clamped.degree, &clamped.knots, knot, count)?;
    let mut candidate = remove(&clamped, &knots)?;
    if candidate.degree != clamped.degree || candidate.knots != knots {
        return Err(GeometryError::KnotRemovalSolveFailed.into());
    }
    restore_constant_channels(original, &mut candidate)?;
    let bound = if let Some(limit) = limit {
        let Some(bound) = original.parameter_curve_deviation_bound(&candidate, limit)? else {
            return Ok(None);
        };
        Some(bound)
    } else {
        None
    };
    Ok(Some((candidate, bound)))
}

fn restore_constant_channels(
    source: &NurbsCurve,
    candidate: &mut NurbsCurve,
) -> Result<(), GeometryError> {
    let first = source.control_points[0];
    let point = first.point().to_array();
    let coordinates: [Option<Real>; 3] = std::array::from_fn(|axis| {
        source
            .control_points
            .iter()
            .all(|control| control.point().to_array()[axis] == point[axis])
            .then_some(point[axis])
    });
    let weight = source
        .control_points
        .iter()
        .all(|control| control.weight() == first.weight())
        .then_some(first.weight());
    for control in &mut candidate.control_points {
        let mut point = control.point().to_array();
        for (axis, constant) in coordinates.iter().enumerate() {
            if let Some(value) = constant {
                point[axis] = *value;
            }
        }
        *control =
            WeightedPoint3::try_new(Point3::try_from(point)?, weight.unwrap_or(control.weight()))?;
    }
    let first_weight = candidate.control_points[0].weight();
    candidate.rational = candidate
        .control_points
        .iter()
        .any(|point| point.weight() != first_weight);
    candidate.rational_representation |= candidate
        .control_points
        .iter()
        .any(|point| point.weight() != 1.);
    Ok(())
}

#[cfg(test)]
mod tests;
