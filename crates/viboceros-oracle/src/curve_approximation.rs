//! Instrument the shared certified cubic producer independently of Rhino Refit.
use super::*;

const MAX_PARAMETERS: usize = 4096;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveCorrespondenceFixture {
    pub curves: [NurbsCurveDefinition; 2],
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
    #[serde(default = "identity_correspondence")]
    pub correspondence: Vec<[f64; 2]>,
}

fn identity_correspondence() -> Vec<[f64; 2]> {
    vec![[0., 0.], [1., 1.]]
}

pub(super) fn deviation(
    fixture: &CurveCorrespondenceFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let a = nurbs_curve_from_definition(&fixture.curves[0])?;
    let b = nurbs_curve_from_definition(&fixture.curves[1])?;
    let before = [
        nurbs_curve_definition_value(&a),
        nurbs_curve_definition_value(&b),
    ];
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let mut result = a.mapped_curve_deviation_bound(&b, &fixture.correspondence, limit)?;
    let started = Instant::now();
    for _ in 0..iterations {
        result = black_box(a.mapped_curve_deviation_bound(&b, &fixture.correspondence, limit)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!({
            "before": before,
            "after": [nurbs_curve_definition_value(&a), nurbs_curve_definition_value(&b)],
            "correspondence": fixture.correspondence,
            "certified": result.is_some(),
            "maximum_deviation_bound": result,
        }),
        elapsed,
    ))
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CubicApproximationFixture {
    pub curve: NurbsCurveDefinition,
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
    #[serde(default)]
    pub parameters: Vec<f64>,
}

pub(super) fn run(
    fixture: &CubicApproximationFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let tolerance = Tolerance::try_new(
        fixture.absolute_tolerance.unwrap_or(tolerance.absolute()),
        tolerance.relative(),
        tolerance.angular(),
    )?;
    if fixture.parameters.len() > MAX_PARAMETERS {
        return Err(ProbeError::FixtureInvariant(
            "cubic approximation accepts at most 4096 sample parameters",
        ));
    }
    let source = nurbs_curve_from_definition(&fixture.curve)?;
    let parameters = if fixture.parameters.is_empty() {
        (0..=32)
            .map(|i| source.parameter_at(f64::from(i) / 32.))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        fixture.parameters.clone()
    };
    for &parameter in &parameters {
        source.evaluate(parameter)?;
    }
    let before = nurbs_curve_definition_value(&source);
    let mut result = source.try_approximate_cubic_with_bound(tolerance)?;
    let started = Instant::now();
    for _ in 0..iterations {
        result = black_box(source.try_approximate_cubic_with_bound(tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let points = parameters
        .iter()
        .map(|&t| result.0.evaluate(t).map(Point3::to_array))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        json!({
            "before":before,
            "after":nurbs_curve_definition_value(&source),
            "curve":nurbs_curve_definition_value(&result.0),
            "maximum_deviation_bound":result.1,
            "parameters":parameters,
            "points":points,
        }),
        elapsed,
    ))
}
