//! Match refitted sets containing corners or regular noncircular loops.
use super::*;
use crate::section_basis::{self, WeightScale};

// Translated or uniformly scaled fits can disagree by more than binary64
// roundoff. Consolidate nearby normalized knots before common-basis insertion.
const MATCHED_KNOT_ROUNDOFF: Real = 2e-8;

enum Profile<'a> {
    Linear(&'a [[Rational; 3]]),
    Open,
    Loop,
    Circle(circle::CircularSection),
}

enum FitReference {
    Linear(NurbsCurve),
    Stations(Vec<Real>),
    Source,
}

pub(super) fn fit_set(
    sections: &[SweepSection],
    linear: &[Option<Vec<[Rational; 3]>>],
    tolerance: Real,
) -> Result<Option<(Vec<NurbsCurve>, Option<Real>)>, GeometryError> {
    let mut profiles = Vec::with_capacity(sections.len());
    let limit = tolerance * 0.5;
    for (section, vertices) in sections.iter().zip(linear) {
        let source = &section.curve;
        if let Some(vertices) = vertices {
            profiles.push(Profile::Linear(vertices));
            continue;
        }
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
    let has_loop = profiles.iter().any(|p| matches!(p, Profile::Loop));
    let has_corners = profiles
        .iter()
        .any(|p| matches!(p, Profile::Linear(vertices) if vertices.len() > 2));
    if !has_loop && !has_corners {
        return Ok(None);
    }
    let has_circle = profiles.iter().any(|p| matches!(p, Profile::Circle(_)));
    let match_regions = if has_corners {
        has_loop || !has_circle
    } else {
        profiles.iter().all(|p| matches!(p, Profile::Loop))
    };
    let (fits, references): (Vec<_>, Vec<_>) = sections
        .iter()
        .zip(&profiles)
        .map(|(section, profile)| match profile {
            Profile::Linear(vertices) => {
                let curve = fit_linear(vertices, limit)?;
                Ok((
                    curve.try_reparameterized(0. ..=1.)?,
                    FitReference::Linear(curve),
                ))
            }
            Profile::Open => Ok((refit(&section.curve, limit, false)?, FitReference::Source)),
            Profile::Loop | Profile::Circle(_) => {
                let keep_regions = match_regions && matches!(profile, Profile::Loop);
                let (curve, parameters) = crate::loft::closed_set_cubic_proposal(
                    &section.curve,
                    tolerance,
                    keep_regions,
                )?
                .ok_or_else(unsupported)?;
                Ok((curve, FitReference::Stations(parameters)))
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?
        .into_iter()
        .unzip();
    let (mut fits, length) = if !match_regions {
        let mut length = 0_f64;
        for ((section, profile), reference) in sections.iter().zip(&profiles).zip(&references) {
            let analytic_length = match (profile, reference) {
                (Profile::Circle(circle), _) => circle.analytic_length(&section.curve),
                (_, FitReference::Linear(curve)) => Some(*curve.domain().end()),
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
    for ((original, fitted), reference) in sections.iter().zip(&fits).zip(references) {
        let bound = match reference {
            FitReference::Linear(curve) => {
                let parameters = curve.knots().chunk_by(|a, b| a == b).map(|g| g[0]);
                let parameters = parameters.collect::<Vec<_>>();
                // The first half of the budget already covered rounding the
                // exact polygon's cubic handles.
                g1::deviation_bound_at_knots(&curve, fitted, &parameters, limit)?
            }
            FitReference::Stations(parameters) => {
                g1::deviation_bound_at_knots(&original.curve, fitted, &parameters, tolerance)?
            }
            FitReference::Source => g1::deviation_bound(&original.curve, fitted, tolerance)?,
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
