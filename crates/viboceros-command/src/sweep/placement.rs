//! Automatic stations retain section order; reversing the rail changes traversal.
use super::*;

pub(super) fn sections(
    mut rail: NurbsCurve,
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, Vec<SweepSection>), CommandError> {
    let mut sections = Vec::with_capacity(profiles.len());
    for curve in profiles {
        #[cfg(feature = "native-smlib")]
        let minimum = {
            let seed = viboceros_smlib::curve_minimum_distance(
                CurveRef::NurbsCurve(&rail),
                CurveRef::NurbsCurve(&curve),
                [[0., 1.]; 2],
                tolerance,
            )
            .map_err(knot_refinement::native_geometry_error)?
            .ok_or(GeometryError::InvalidSweep {
                context: "could not place a section on the rail",
            })?;
            rail.minimum_distance_with_curve_seeded(&curve, tolerance, seed.parameters)?
        };
        #[cfg(not(feature = "native-smlib"))]
        let minimum = rail.minimum_distance_with_curve(&curve, tolerance)?;
        let minimum = minimum.ok_or(GeometryError::InvalidSweep {context:"section placement exceeded the certified curve-distance query limits; supply explicit Parameters"})?;
        sections.push(SweepSection {
            parameter: minimum.parameters[0],
            curve,
        });
    }
    let reverse = if sections.len() == 1 {
        if rail.is_closed()? {
            false
        } else {
            let sampler = viboceros_geometry::ArcLengthSampler::try_new(
                CurveRef::NurbsCurve(&rail),
                tolerance,
            )?;
            let distance = sampler.distance_at_parameter(sections[0].parameter)?;
            distance > sampler.total_length() * (0.5 + 8. * Real::EPSILON)
        }
    } else {
        sections[0].parameter > sections[1].parameter
    };
    if reverse {
        rail = rail.reversed()?;
        for section in &mut sections {
            section.parameter = -section.parameter;
        }
    }
    Ok((rail, sections))
}

pub(super) fn full_rail_anchor(
    rail: &NurbsCurve,
    sections: &[SweepSection],
) -> Result<bool, GeometryError> {
    if sections.len() != 1 {
        return Ok(false);
    }
    let domain = rail.domain();
    let t = sections[0].parameter;
    if t <= *domain.start() || t >= *domain.end() {
        return Ok(false);
    }
    let scale = domain.end() - domain.start();
    if !scale.is_finite() {
        return Ok(false);
    }
    Ok(rail
        .greville_parameters_in_active_domain()?
        .into_iter()
        .any(|g| (g - t).abs() <= 64. * Real::EPSILON * scale))
}
