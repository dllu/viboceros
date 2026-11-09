//! Tensor basis reduction with a complete bound against the original surface.
use super::*;
use crate::MAX_KNOT_REDUCTION_GROUPS;
mod basis;

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceKnotReduction {
    pub surface: NurbsSurface,
    /// Total knots removed from the selected U/V vectors, not control points.
    pub removed: usize,
    /// Complete original-surface bound; `None` denotes unrestricted removal.
    pub maximum_deviation_bound: Option<Real>,
}

impl NurbsSurface {
    /// Reduce the exact selected interior knot to at most `target_multiplicity`.
    /// Requires U or V. `Some(result)` certifies the entire original tensor at
    /// `limit`; `None` declines an outside or unproved proposal. Zero is strict.
    pub fn try_reduce_knot_multiplicity_with_bound(
        &self,
        direction: SurfaceKnotDirection,
        knot: Real,
        target_multiplicity: usize,
        limit: Real,
    ) -> Result<Option<SurfaceKnotReduction>, GeometryError> {
        if direction == SurfaceKnotDirection::Both {
            return Err(GeometryError::InvalidControlNet {
                context: "knot reduction requires one surface direction",
            });
        }
        validate(self, direction, Some(limit))?;
        let count = groups(self, direction)
            .into_iter()
            .find_map(|(value, count)| (value == knot).then_some(count))
            .ok_or(GeometryError::NoRemovableKnot { parameter: knot })?;
        if target_multiplicity >= count {
            return Ok(Some(unchanged(self, Some(limit))));
        }
        attempt(
            self,
            self,
            direction,
            knot,
            count - target_multiplicity,
            Some(limit),
            &mut basis::fit,
        )
        .map(|proposal| {
            proposal.map(|(surface, bound)| SurfaceKnotReduction {
                surface,
                removed: count - target_multiplicity,
                maximum_deviation_bound: bound,
            })
        })
    }

    /// Collapse groups with multiplicity strictly between `minimum` and
    /// `maximum` to simple knots. Degree-one knots disappear; minimum zero is
    /// treated like one for higher degrees. Process descending U groups, then
    /// descending V groups. Every candidate is bounded against `self`, across
    /// both directions. `Some(0.)` is strict; `None` permits unrestricted edits.
    /// Periodic selected directions and >256 eligible groups fail before work.
    /// Bounded edits require degree <=16, <=4096 controls and common-sign
    /// nonzero weights. Inconclusive certificates decline the candidate.
    pub fn try_reduce_knots_in_range(
        &self,
        direction: SurfaceKnotDirection,
        minimum: usize,
        maximum: usize,
        limit: Option<Real>,
    ) -> Result<SurfaceKnotReduction, GeometryError> {
        self.try_reduce_knots_in_range_with_remover(
            direction,
            minimum,
            maximum,
            limit,
            &mut basis::fit,
        )
    }

    /// Delegate homogeneous fitting while retaining selection, source ownership
    /// and certification. The callback must return exactly the requested basis
    /// and leave the other parameter direction unchanged.
    pub fn try_reduce_knots_in_range_with_remover<E: From<GeometryError>>(
        &self,
        direction: SurfaceKnotDirection,
        minimum: usize,
        maximum: usize,
        limit: Option<Real>,
        remove: &mut impl FnMut(&Self, SurfaceKnotDirection, &[Real]) -> Result<Self, E>,
    ) -> Result<SurfaceKnotReduction, E> {
        if minimum >= maximum {
            return Err(GeometryError::InvalidKnotReductionRange { minimum, maximum }.into());
        }
        validate(self, direction, limit)?;
        let mut work = Vec::new();
        for axis in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
            if direction != SurfaceKnotDirection::Both && direction != axis {
                continue;
            }
            let degree = if axis == SurfaceKnotDirection::U {
                self.degree_u
            } else {
                self.degree_v
            };
            work.extend(
                groups(self, axis)
                    .into_iter()
                    .rev()
                    .filter_map(|(knot, count)| {
                        (count < maximum
                            && if degree == 1 {
                                minimum <= 1
                            } else {
                                count > minimum.max(1)
                            })
                        .then_some((
                            axis,
                            knot,
                            if degree == 1 { count } else { count - 1 },
                        ))
                    }),
            );
        }
        if work.len() > MAX_KNOT_REDUCTION_GROUPS {
            return Err(GeometryError::SurfaceApproximationLimit.into());
        }
        let mut result = unchanged(self, limit);
        for (axis, knot, count) in work {
            if let Some((surface, bound)) =
                attempt(self, &result.surface, axis, knot, count, limit, remove)?
            {
                result.surface = surface;
                result.removed += count;
                result.maximum_deviation_bound = bound;
            }
        }
        Ok(result)
    }
}

fn validate(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    limit: Option<Real>,
) -> Result<(), GeometryError> {
    if limit.is_some_and(|t| !t.is_finite() || t < 0.) {
        return Err(GeometryError::InvalidTolerance);
    }
    if source.control_points.len() > 1_000_000 {
        return Err(GeometryError::SurfaceApproximationLimit);
    }
    for (selected, periodic, name) in [
        (
            direction != SurfaceKnotDirection::V,
            source.is_periodic_u(),
            "U",
        ),
        (
            direction != SurfaceKnotDirection::U,
            source.is_periodic_v(),
            "V",
        ),
    ] {
        if selected && periodic {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: name });
        }
    }
    if limit.is_some()
        && (source.degree_u > 16
            || source.degree_v > 16
            || source.control_points.len() > 4096
            || source.control_points.iter().any(|p| {
                p.weight() == 0.
                    || p.weight().is_sign_negative()
                        != source.control_points[0].weight().is_sign_negative()
            }))
    {
        return Err(GeometryError::UnsupportedSurfaceApproximation {
            context: "bounded knot reduction requires degrees <=16, <=4096 controls and nonzero common-sign weights",
        });
    }
    Ok(())
}
fn groups(source: &NurbsSurface, direction: SurfaceKnotDirection) -> Vec<(Real, usize)> {
    let (knots, domain) = if direction == SurfaceKnotDirection::U {
        (&source.knots_u, source.domain_u())
    } else {
        (&source.knots_v, source.domain_v())
    };
    knots
        .chunk_by(|a, b| a == b)
        .filter(|g| g[0] > *domain.start() && g[0] < *domain.end())
        .map(|g| (g[0], g.len()))
        .collect()
}
fn unchanged(surface: &NurbsSurface, limit: Option<Real>) -> SurfaceKnotReduction {
    SurfaceKnotReduction {
        surface: surface.clone(),
        removed: 0,
        maximum_deviation_bound: limit.map(|_| 0.),
    }
}
fn attempt<E: From<GeometryError>>(
    original: &NurbsSurface,
    current: &NurbsSurface,
    axis: SurfaceKnotDirection,
    knot: Real,
    count: usize,
    limit: Option<Real>,
    remove: &mut impl FnMut(&NurbsSurface, SurfaceKnotDirection, &[Real]) -> Result<NurbsSurface, E>,
) -> Result<Option<(NurbsSurface, Option<Real>)>, E> {
    let clamp = |curve: &NurbsCurve| curve.clamped_to_active_domain();
    let base = if axis == SurfaceKnotDirection::U {
        current.map_u_control_curves(clamp)?
    } else {
        current.map_v_control_curves(clamp)?
    };
    let (degree, before) = if axis == SurfaceKnotDirection::U {
        (base.degree_u, &base.knots_u)
    } else {
        (base.degree_v, &base.knots_v)
    };
    let knots = crate::knot_group_removal_knots(degree, before, knot, count)?;
    if (knots.len() - degree - 1)
        .checked_pow(3)
        .is_none_or(|work| work > 64_000_000)
    {
        return Err(GeometryError::SurfaceApproximationLimit.into());
    }
    let candidate = remove(&base, axis, &knots)?;
    if candidate.degree_u != base.degree_u
        || candidate.degree_v != base.degree_v
        || candidate.knots_u
            != if axis == SurfaceKnotDirection::U {
                knots.as_slice()
            } else {
                base.knots_u.as_slice()
            }
        || candidate.knots_v
            != if axis == SurfaceKnotDirection::V {
                knots.as_slice()
            } else {
                base.knots_v.as_slice()
            }
    {
        return Err(GeometryError::KnotRemovalSolveFailed.into());
    }
    let candidate = basis::restore(&base, &candidate, axis)?;
    let bound = if let Some(limit) = limit {
        let Some(bound) = original.parameter_surface_deviation_bound(&candidate, limit)? else {
            return Ok(None);
        };
        Some(bound)
    } else {
        None
    };
    Ok(Some((candidate, bound)))
}

#[cfg(test)]
mod backend_tests;
#[cfg(test)]
mod tests;
