//! SDK range comparisons backed by complete original-tensor displacement bounds.
use super::*;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    U,
    V,
    #[default]
    Both,
}
impl Direction {
    fn geometry(self) -> SurfaceKnotDirection {
        match self {
            Self::U => SurfaceKnotDirection::U,
            Self::V => SurfaceKnotDirection::V,
            Self::Both => SurfaceKnotDirection::Both,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: NurbsSurfaceDefinition,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default = "minimum")]
    pub minimum_multiplicity: usize,
    #[serde(default)]
    pub maximum_multiplicity: Option<usize>,
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}
fn minimum() -> usize {
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
    let source = nurbs_surface_from_definition(&fixture.surface)?;
    let maximum = fixture
        .maximum_multiplicity
        .unwrap_or(source.degree_u().max(source.degree_v()) + 1);
    let build = || -> Result<viboceros_geometry::SurfaceKnotReduction, ProbeError> {
        // The SDK-facing zero sentinel is separate from the kernel's strict Some(0).
        let bound = (limit > 0.).then_some(limit);
        #[cfg(feature = "native-smlib")]
        {
            viboceros_smlib::reduce_surface_knots_in_range(
                &source,
                fixture.direction.geometry(),
                fixture.minimum_multiplicity,
                maximum,
                bound,
            )
            .map_err(CommandError::from)
            .map_err(ProbeError::from)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            Ok(source.try_reduce_knots_in_range(
                fixture.direction.geometry(),
                fixture.minimum_multiplicity,
                maximum,
                bound,
            )?)
        }
    };
    let mut result = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        result = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let mut samples = Vec::with_capacity(81);
    for i in 0..=8 {
        for j in 0..=8 {
            samples.push(
                result
                    .surface
                    .evaluate(
                        result.surface.parameter_at_u(i as f64 / 8.)?,
                        result.surface.parameter_at_v(j as f64 / 8.)?,
                    )?
                    .to_array(),
            );
        }
    }
    Ok((
        json!({"removed":result.removed,"surface":nurbs_surface_definition_value(&result.surface),"samples":samples,
        "before":nurbs_surface_definition_value(&source),"after":nurbs_surface_definition_value(&source)}),
        elapsed,
    ))
}
