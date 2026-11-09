//! Polynomial Greville offsets for Python instrumentation, with original charts.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub distance: f64,
    #[serde(default = "normal")]
    pub normal: [f64; 3],
}
fn normal() -> [f64; 3] {
    [0., 0., 1.]
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_curve_from_definition(&fixture.curve)?;
    let normal = Vector3::try_from(fixture.normal)?.normalized_nonzero()?;
    #[cfg(feature = "native-smlib")]
    let solver = Some(
        viboceros_smlib::curve_edit_point_interpolation as viboceros_geometry::CurveEditPointSolver,
    );
    #[cfg(not(feature = "native-smlib"))]
    let solver = None;
    let (curve, elapsed) = measure_probe(iterations, || {
        source.try_loose_polynomial_offset(fixture.distance, normal, tolerance, solver)
    })?;
    Ok((
        json!({"before":nurbs_curve_definition_value(&source),"after":nurbs_curve_definition_value(&source),
        "curve":nurbs_curve_definition_value(&curve),"distance_certified":false}),
        elapsed,
    ))
}
