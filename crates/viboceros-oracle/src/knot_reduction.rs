//! Compare SDK multiplicity selection with continuously bounded basis edits.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    #[serde(default = "default_minimum")]
    pub minimum_multiplicity: usize,
    #[serde(default)]
    pub maximum_multiplicity: Option<usize>,
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}

fn default_minimum() -> usize {
    1
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance.into());
    }
    if fixture
        .maximum_multiplicity
        .is_some_and(|maximum| maximum <= fixture.minimum_multiplicity)
    {
        return Err(GeometryError::InvalidKnotReductionRange {
            minimum: fixture.minimum_multiplicity,
            maximum: fixture.maximum_multiplicity.unwrap(),
        }
        .into());
    }
    let source = nurbs_curve_from_definition(&fixture.curve)?;
    let maximum = fixture.maximum_multiplicity.unwrap_or(source.degree() + 1);
    let before = nurbs_curve_definition_value(&source);
    // The captured SDK treats zero as unrestricted. The Rust kernel keeps
    // Some(0) strict; only this SDK-facing protocol translates the sentinel.
    let bound = (limit > 0.).then_some(limit);
    let build = || -> Result<viboceros_geometry::KnotReduction, ProbeError> {
        #[cfg(feature = "native-smlib")]
        {
            viboceros_smlib::reduce_curve_knots_in_range(
                &source,
                fixture.minimum_multiplicity,
                maximum,
                bound,
            )
            .map_err(CommandError::from)
            .map_err(ProbeError::from)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            Ok(source.try_reduce_knots_in_range(fixture.minimum_multiplicity, maximum, bound)?)
        }
    };
    let mut result = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        result = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let samples = (0..=32)
        .map(|i| {
            result
                .curve
                .evaluate(result.curve.parameter_at(f64::from(i) / 32.)?)
                .map(Point3::to_array)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok((
        json!({"removed":result.removed,"curve":nurbs_curve_definition_value(&result.curve),
        "samples":samples,"before":before,"after":nurbs_curve_definition_value(&source)}),
        elapsed,
    ))
}
