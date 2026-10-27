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
        context: "Loft Refit currently requires open regular profiles, tolerance below their endpoint chords, and a complete adaptive source certificate",
    }
}

fn adaptive(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Prepared, GeometryError> {
    let mut fitted = Vec::with_capacity(curves.len());
    let mut deviations = Vec::with_capacity(curves.len());
    for source in curves {
        let domain = source.domain();
        // Large relative tolerances enter a distinct native resampling branch.
        if tolerance
            >= source
                .evaluate(*domain.start())?
                .distance_to(source.evaluate(*domain.end())?)?
        {
            return Err(unsupported());
        }
        let Some(proposal) =
            adaptive::certified_joined_refit(source, tolerance, angle.unwrap_or(0.))?
        else {
            return Err(unsupported());
        };
        fitted.push(proposal.curve);
        deviations.push(proposal.maximum_deviation_bound);
    }
    Ok(Prepared {
        curves: fitted,
        mode: Mode::Adaptive,
        deviations,
    })
}

fn initial(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Option<Real>,
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
            if curves.len() >= 2 && angle.is_none_or(|a| a <= std::f64::consts::FRAC_PI_2) =>
        {
            return candidate::prepare(curves, fit_tolerance, angle).map(|curves| Prepared {
                curves,
                mode: Mode::Initial,
                deviations: Vec::new(),
            });
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
        deviations: Vec::new(),
    })
}

pub(super) fn propose(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Prepared, GeometryError> {
    match initial(curves, tolerance, angle) {
        Err(GeometryError::UnsupportedCurveApproximation { .. })
            if curves.len() >= 2 && angle.is_none_or(|a| a <= std::f64::consts::FRAC_PI_2) =>
        {
            adaptive(curves, tolerance, angle)
        }
        result => result,
    }
}

mod preservation;

#[cfg(test)]
mod tests;
