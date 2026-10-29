//! Retain the fitting path through common region and basis preparation.
use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    Original,
    Initial,
    Adaptive,
}

pub(super) struct Prepared {
    pub curves: Vec<NurbsCurve>,
    pub mode: Mode,
    pub smooth: bool,
    deviations: Vec<Real>,
}

impl Prepared {
    pub fn finish<E: From<GeometryError>>(
        self,
        sources: &[NurbsCurve],
        tolerance: Real,
        angle: Option<Real>,
        clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
        elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
        refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
    ) -> Result<Vec<NurbsCurve>, E> {
        let charts = if self.mode == Mode::Adaptive {
            Some(super::super::parameterize::prepare_fitted(
                &self.curves,
                clamp,
            )?)
        } else {
            None
        };
        let result = match self.mode {
            Mode::Original => compatible::prepare_refit_with_operations(
                &self.curves,
                angle,
                clamp,
                elevate,
                refine,
            ),
            Mode::Initial => {
                compatible::prepare_with_operations(&self.curves, clamp, elevate, refine)
            }
            Mode::Adaptive => {
                compatible::match_basis(charts.as_ref().unwrap(), clamp, elevate, refine)
            }
        }?;
        if let Some(charts) = charts {
            for ((((source, prior), chart), matched), &deviation) in sources
                .iter()
                .zip(&self.curves)
                .zip(charts.iter())
                .zip(&result)
                .zip(&self.deviations)
            {
                // An identical curve retains the complete proposal certificate.
                // Otherwise compose its bound with a complete bound on the
                // basis edits, charging both against the original tolerance.
                // If that cannot be proved, search directly from the source.
                if prior != matched
                    && !preservation::within_remaining_deviation(
                        prior, chart, matched, deviation, tolerance,
                    )?
                    && !crate::curve_fit::g1::within_deviation(source, matched, tolerance)?
                {
                    return Err(unsupported().into());
                }
            }
        }
        Ok(result)
    }
}

fn unsupported() -> GeometryError {
    GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires regular tangent regions and a complete adaptive source certificate within the fitting budgets",
    }
}

fn adaptive(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
    scope: FitScope,
) -> Result<Prepared, GeometryError> {
    let mut fitted = Vec::with_capacity(curves.len());
    let mut deviations = Vec::with_capacity(curves.len());
    let mut reconstruction_sources = Vec::with_capacity(curves.len());
    for source in curves {
        let Some(proposal) = adaptive::certified_joined_refit_with_scope(
            source,
            tolerance,
            angle.unwrap_or(0.),
            scope.policy,
        )?
        else {
            return Err(unsupported());
        };
        fitted.push(proposal.curve);
        deviations.push(proposal.maximum_deviation_bound);
        reconstruction_sources.push(proposal.reconstruction_source);
    }
    if curves.len() > 1 && scope.policy.geometric_rebuild {
        let mut cuts = fitted
            .iter()
            .flat_map(|c| c.knots().iter().copied())
            .collect::<Vec<_>>();
        cuts.sort_by(Real::total_cmp);
        cuts.dedup();
        if cuts.len() > MAX_LOFT_SECTION_CONTROLS / 2 {
            return Err(GeometryError::LoftResourceLimit {
                context: "shared reconstruction controls",
                maximum: MAX_LOFT_SECTION_CONTROLS,
            });
        }
        for (i, proxy) in reconstruction_sources.iter().enumerate() {
            let Some(proxy) = proxy else {
                continue;
            };
            let own = fitted[i]
                .knots()
                .chunk_by(|a, b| a == b)
                .map(|g| g[0])
                .collect::<Vec<_>>();
            if own == cuts {
                continue;
            }
            let rebuilt = adaptive::reconstruct_at_cuts(proxy, &cuts)?;
            let Some(bound) = crate::curve_fit::g1::reconstruction_deviation_bound(
                &curves[i], &rebuilt, tolerance,
            )?
            else {
                return Err(unsupported());
            };
            fitted[i] = rebuilt;
            deviations[i] = bound;
        }
    }
    Ok(Prepared {
        curves: fitted,
        mode: Mode::Adaptive,
        smooth: true,
        deviations,
    })
}

fn initial(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Option<Real>,
    scope: FitScope,
) -> Result<Prepared, GeometryError> {
    validate_angle(angle)?;
    if !fit_tolerance.is_finite() || fit_tolerance < 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if curves.len() > MAX_LOFT_SECTIONS {
        return Err(GeometryError::LoftResourceLimit {
            context: "refitted section count",
            maximum: MAX_LOFT_SECTIONS,
        });
    }
    if fitting_disabled(fit_tolerance) {
        return Ok(Prepared {
            curves: curves.to_vec(),
            mode: Mode::Original,
            smooth: false,
            deviations: Vec::new(),
        });
    }
    let sections = curves
        .iter()
        .map(|source| match straight(source, fit_tolerance) {
            Err(GeometryError::UnsupportedCurveApproximation { .. }) => {
                polyline::refit(source, fit_tolerance, angle)
            }
            Ok(_) if angle.is_some_and(|a| a > std::f64::consts::FRAC_PI_2) => {
                Err(GeometryError::UnsupportedCurveApproximation {
                    context: "Loft Refit currently requires straight-section angles at most pi/2",
                })
            }
            result => result,
        })
        .collect::<Result<Vec<_>, GeometryError>>();
    let sections = match sections {
        Ok(sections) => sections,
        Err(GeometryError::UnsupportedCurveApproximation { .. })
            if curves.len() >= scope.minimum_curves
                && angle.is_none_or(|a| a <= std::f64::consts::FRAC_PI_2) =>
        {
            return candidate::prepare_with_policy(curves, fit_tolerance, angle, scope.policy).map(
                |curves| Prepared {
                    curves,
                    mode: Mode::Initial,
                    smooth: true,
                    deviations: Vec::new(),
                },
            );
        }
        Err(error) => return Err(error),
    };
    if let Some(first) = sections.first() {
        let closed = first.curve.is_closed()?;
        for section in &sections[1..] {
            if section.curve.is_closed()? != closed {
                return Err(GeometryError::UnsupportedCurveApproximation {
                    context: "Loft Refit currently requires profiles with matching open/closed state",
                });
            }
        }
    }
    Ok(Prepared {
        curves: matching::prepare(sections, fit_tolerance, angle)?,
        mode: Mode::Initial,
        smooth: false,
        deviations: Vec::new(),
    })
}

pub(super) fn propose(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Prepared, GeometryError> {
    propose_with_scope(curves, tolerance, angle, FitScope::LOFT)
}

pub(super) fn propose_with_scope(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
    scope: FitScope,
) -> Result<Prepared, GeometryError> {
    match initial(curves, tolerance, angle, scope) {
        Err(GeometryError::UnsupportedCurveApproximation { .. })
            if curves.len() >= scope.minimum_curves
                && angle.is_none_or(|a| a <= std::f64::consts::FRAC_PI_2) =>
        {
            adaptive(curves, tolerance, angle, scope)
        }
        result => result,
    }
}

mod preservation;

#[cfg(test)]
mod tests;
