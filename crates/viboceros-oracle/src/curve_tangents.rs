//! Ordered tangent batches through the existing owned OpenNURBS query.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveTangentsFixture {
    pub curve: NurbsCurveDefinition,
    pub parameters: Vec<f64>,
}

pub(super) fn run(
    fixture: &CurveTangentsFixture,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    let definition = &fixture.curve;
    let count = definition.control_points.len();
    let degree = definition.degree;
    if !(1..=16).contains(&degree)
        || count <= degree
        || count > 512
        || definition.knots.len() != count + degree + 1
        || !(1..=4096).contains(&fixture.parameters.len())
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid curve tangent dimensions or query count",
        ));
    }
    let [start, end] = definition
        .domain
        .unwrap_or([definition.knots[degree], definition.knots[count]]);
    if !start.is_finite()
        || !end.is_finite()
        || start >= end
        || fixture
            .parameters
            .iter()
            .any(|t| !t.is_finite() || *t < start || *t > end)
    {
        return Err(ProbeError::FixtureInvariant(
            "curve tangent parameters are outside the active domain",
        ));
    }
    let source = nurbs_curve_from_definition(definition)?;
    let before = nurbs_curve_definition_value(&source);
    let controls = source
        .control_points()
        .iter()
        .map(|p| {
            let q = p.point().to_array();
            [q[0], q[1], q[2], p.weight()]
        })
        .collect::<Vec<_>>();
    let (tangents, elapsed) = measure_probe(iterations, || {
        viboceros_opennurbs::curve_tangents(
            degree,
            &controls,
            source.knots(),
            black_box(&fixture.parameters),
        )
        .map_err(GeometryError::from)
        .map_err(ProbeError::from)
    })?;
    Ok((
        json!({"before":before,"after":nurbs_curve_definition_value(&source),
        "parameters":fixture.parameters,"tangents":tangents}),
        elapsed,
    ))
}
