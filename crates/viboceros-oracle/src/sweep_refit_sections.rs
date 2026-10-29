//! Section preparation without rail frames or surface construction.
use super::*;
use viboceros_geometry::SweepSection;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SweepRefitSectionsFixture {
    pub sections: Vec<NurbsCurveDefinition>,
    pub parameters: Vec<f64>,
    pub refit_tolerance: f64,
}

pub(super) fn run(
    fixture: &SweepRefitSectionsFixture,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=256).contains(&fixture.sections.len())
        || fixture.parameters.len() != fixture.sections.len()
        || fixture.parameters.iter().any(|t| !t.is_finite())
        || fixture.parameters.windows(2).any(|p| p[0] >= p[1])
        || !fixture.refit_tolerance.is_finite()
        || fixture.refit_tolerance < 0.
        || fixture.sections.iter().any(|curve| {
            !(1..=16).contains(&curve.degree)
                || curve.control_points.len() <= curve.degree
                || curve.control_points.len() > 512
                || curve.knots.len() != curve.control_points.len() + curve.degree + 1
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid section fitting count, parameters or tolerance",
        ));
    }
    let sections = fixture
        .sections
        .iter()
        .zip(&fixture.parameters)
        .map(|(curve, &parameter)| {
            Ok(SweepSection {
                parameter,
                curve: nurbs_curve_from_definition(curve)?,
            })
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let definitions = |sections: &[SweepSection]| {
        sections
            .iter()
            .map(|section| nurbs_curve_definition_value(&section.curve))
            .collect::<Vec<_>>()
    };
    let before = definitions(&sections);
    let (curves, elapsed) = measure_probe(iterations, || {
        #[cfg(feature = "native-smlib")]
        let result =
            viboceros_smlib::refit_sweep_sections(&sections, black_box(fixture.refit_tolerance))
                .map_err(CommandError::from)
                .map_err(ProbeError::from);
        #[cfg(not(feature = "native-smlib"))]
        let result = viboceros_geometry::Sweep1::refit_sections(
            &sections,
            black_box(fixture.refit_tolerance),
        )
        .map_err(ProbeError::from);
        result
    })?;
    Ok((
        json!({"before":before,"after":definitions(&sections),
        "curves":definitions(&curves),
        "parameters":curves.iter().map(|section| section.parameter).collect::<Vec<_>>()}),
        elapsed,
    ))
}
