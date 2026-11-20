//! Match refitted sets containing regular noncircular closed profiles.
use super::*;
use crate::section_basis::{self, WeightScale};

// Translated or uniformly scaled fits can disagree by more than binary64
// roundoff. Consolidate nearby normalized knots before common-basis insertion.
const MATCHED_KNOT_ROUNDOFF: Real = 2e-8;

pub(super) fn fit_set(
    sections: &[SweepSection],
    tolerance: Real,
) -> Result<Option<(Vec<NurbsCurve>, Option<Real>)>, GeometryError> {
    enum Profile {
        Open,
        Loop,
        Circle(circle::CircularSection),
    }
    let mut profiles = Vec::with_capacity(sections.len());
    let limit = tolerance * 0.5;
    for section in sections {
        let source = &section.curve;
        if !regular_degree_and_weights(source) {
            return Ok(None);
        }
        let profile = if source.is_closed()? {
            circle::recognize(source, limit)?.map_or(Profile::Loop, Profile::Circle)
        } else {
            Profile::Open
        };
        profiles.push(profile);
    }
    if !profiles
        .iter()
        .any(|profile| matches!(profile, Profile::Loop))
    {
        return Ok(None);
    }
    for section in sections {
        if crate::surface_pullback::exact_linear_curve_vertices(&section.curve)?.is_some() {
            return Ok(None);
        }
    }
    let mixed = profiles
        .iter()
        .any(|profile| !matches!(profile, Profile::Loop));
    let (fits, stations): (Vec<_>, Vec<_>) = sections
        .iter()
        .zip(&profiles)
        .map(|(section, profile)| {
            if matches!(profile, Profile::Open) {
                Ok((refit(&section.curve, limit, false)?, None))
            } else {
                let (curve, parameters) =
                    crate::loft::closed_set_cubic_proposal(&section.curve, tolerance, !mixed)?
                        .ok_or_else(unsupported)?;
                Ok((curve, Some(parameters)))
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?
        .into_iter()
        .unzip();
    let (mut fits, length) = if mixed {
        let mut length = 0_f64;
        for (section, profile) in sections.iter().zip(&profiles) {
            let analytic_length = match profile {
                Profile::Circle(circle) => circle.analytic_length(&section.curve),
                _ => None,
            };
            let chart = match analytic_length {
                Some(length) => length,
                None => fit_length(&section.curve)?,
            };
            length = length.max(chart);
        }
        (fits, length)
    } else if let Some(matched) = crate::loft::prepare_sweep_refit_regions(&fits)? {
        matched
    } else {
        let fits = shared::reconstruct(sections, fits, tolerance, shared::Subdivision::Image)?;
        return Ok(Some((fits, None)));
    };
    crate::loft::consolidate_section_knots(&mut fits, MATCHED_KNOT_ROUNDOFF)?;
    for ((original, fitted), parameters) in sections.iter().zip(&fits).zip(stations) {
        let bound = if let Some(parameters) = parameters {
            g1::deviation_bound_at_knots(&original.curve, fitted, &parameters, tolerance)?
        } else {
            g1::deviation_bound(&original.curve, fitted, tolerance)?
        };
        if bound.is_none() {
            return Err(unsupported());
        }
    }
    // Match the common basis here so the ordinary Sweep1 section preparer
    // keeps the measured regional correspondence.
    let compatible = section_basis::prepare(
        &fits,
        MAX_SECTION_CONTROLS,
        invalid("refit compatible section control budget exceeded"),
        WeightScale::Preserve,
    )?;
    Ok(Some((compatible, Some(length))))
}
