//! Place profiles around one circuit starting at the first profile.
use super::*;
use viboceros_geometry::wrap_closed_curve_parameter;

pub(super) const REQUIREMENTS: &str =
    "Sweep1 Closed=Yes requires a closed rail, at least two profiles, and no point ends";

pub(super) fn sections(
    source: CurveRef<'_>,
    curves: Vec<NurbsCurve>,
    explicit: Option<&[Real]>,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, Vec<SweepSection>, bool), CommandError> {
    let mut rail = source.to_nurbs()?;
    let mut sections = if let Some(parameters) = explicit {
        if parameters.len() != curves.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let domain = source.domain();
        let roundoff = 8. * Real::EPSILON * (domain.end() - domain.start());
        parameters
            .iter()
            .zip(curves)
            .map(|(&t, curve)| {
                let snapped = t.clamp(*domain.start(), *domain.end());
                if !t.is_finite() || (snapped - t).abs() > roundoff {
                    return Err(GeometryError::InvalidSweep {
                        context: "closed sweep station outside the rail domain",
                    });
                }
                Ok(SweepSection {
                    parameter: source.nurbs_parameter(snapped)?,
                    curve,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?
    } else {
        placement::profile_contacts(CurveRef::NurbsCurve(&rail), &rail, curves, tolerance)?
    };
    let domain = rail.domain();
    let period = domain.end() - domain.start();
    let active_knots = &rail.knots()[rail.degree()..=rail.control_points().len()];
    for section in &mut sections {
        let parameter = section.parameter;
        let nearest = active_knots.partition_point(|&knot| knot < parameter);
        if let Some(knot) = [nearest.checked_sub(1), Some(nearest)]
            .into_iter()
            .flatten()
            .filter_map(|i| active_knots.get(i).copied())
            .find(|&knot| (knot - parameter).abs() <= 64. * Real::EPSILON * period)
        {
            section.parameter = knot;
        }
        section.parameter = wrap_closed_curve_parameter(section.parameter, &domain)?;
    }
    let refit = sections.len() > 1
        && sections.iter().any(|section| {
            active_knots
                .binary_search_by(|knot| knot.total_cmp(&section.parameter))
                .is_err()
        });
    let seam = sections[0].parameter;
    let phase = |delta: Real| delta.rem_euclid(period);
    let ordered = |reverse: bool| {
        let station = |section: &SweepSection| {
            let delta = section.parameter - seam;
            phase(if reverse { -delta } else { delta })
        };
        sections
            .windows(2)
            .all(|pair| station(&pair[0]) < station(&pair[1]))
    };
    if !ordered(false) {
        if !ordered(true) {
            return Err(CommandError::Usage(
                "Sweep1 profiles must progress in order around the closed rail",
            ));
        }
        rail = rail.reversed()?;
        for section in &mut sections {
            section.parameter = wrap_closed_curve_parameter(-section.parameter, &rail.domain())?;
        }
    }
    let seam = sections[0].parameter;
    rail = viboceros_smlib::change_curve_seam(&rail, seam)
        .map_err(knot_refinement::native_geometry_error)?;
    for section in &mut sections {
        if section.parameter < seam {
            section.parameter += period;
        }
    }
    Ok((rail, sections, refit))
}

pub(super) fn split_parameters(surface: &NurbsSurface) -> Vec<Real> {
    let domain = surface.domain_u();
    surface
        .knots_u()
        .chunk_by(|a, b| a == b)
        .filter(|group| {
            group.len() >= surface.degree_u()
                && group[0] > *domain.start()
                && group[0] < *domain.end()
        })
        .map(|group| group[0])
        .collect()
}
