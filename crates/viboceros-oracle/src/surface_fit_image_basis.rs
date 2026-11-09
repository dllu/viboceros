//! Explicit polynomial basis and supplied-map image admission, local only.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: NurbsSurfaceDefinition,
    pub degree_u: usize,
    pub degree_v: usize,
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    pub correspondence: [Vec<[f64; 2]>; 2],
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_surface_from_definition(&fixture.surface)?;
    let degree = [fixture.degree_u, fixture.degree_v];
    let knots = [fixture.knots_u.as_slice(), fixture.knots_v.as_slice()];
    let maps = fixture.correspondence.each_ref().map(Vec::as_slice);
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let build = || -> Result<_, ProbeError> {
        #[cfg(feature = "native-smlib")]
        {
            viboceros_smlib::fit_surface_image_in_basis(&source, degree, knots, maps, limit)
                .map_err(CommandError::from)
                .map_err(ProbeError::from)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            Ok(viboceros_geometry::try_fit_surface_image_in_basis(
                &source, degree, knots, maps, limit,
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
    Ok((
        json!({"surface":result.as_ref().map(|r|nurbs_surface_definition_value(&r.surface)),
        "maximum_deviation_bound":result.as_ref().map(|r|r.maximum_deviation_bound),
        "before":nurbs_surface_definition_value(&source),"after":nurbs_surface_definition_value(&source)}),
        elapsed,
    ))
}
