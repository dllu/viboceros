//! Readonly loose and ordinary smooth ribbon geometry; no command-state claim.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    curve: NurbsCurveDefinition,
    distance: f64,
    #[serde(default)]
    both_sides: bool,
    #[serde(default = "normal")]
    normal: [f64; 3],
}
fn normal() -> [f64; 3] {
    [0., 0., 1.]
}
pub(super) fn run(
    loose: bool,
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_curve_from_definition(&f.curve)?;
    let normal = Vector3::try_from(f.normal)?.normalized_nonzero()?;
    let (brep, elapsed) = measure_probe(iterations, || -> Result<Brep, ProbeError> {
        #[cfg(feature = "native-smlib")]
        {
            if !loose {
                return Ok(viboceros_geometry::try_smooth_ribbon_with_ruled(
                    &source,
                    f.distance,
                    f.both_sides,
                    normal,
                    tolerance,
                    &mut viboceros_smlib::ruled_surface,
                )
                .map_err(CommandError::from)?);
            }
            Ok(viboceros_geometry::try_loose_ribbon_with_ruled(
                &source,
                f.distance,
                f.both_sides,
                normal,
                tolerance,
                Some(viboceros_smlib::curve_edit_point_interpolation),
                &mut viboceros_smlib::ruled_surface,
            )
            .map_err(CommandError::from)?)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            if !loose {
                return Ok(viboceros_geometry::try_smooth_ribbon(
                    &source,
                    f.distance,
                    f.both_sides,
                    normal,
                    tolerance,
                )?);
            }
            Ok(viboceros_geometry::try_loose_ribbon(
                &source,
                f.distance,
                f.both_sides,
                normal,
                tolerance,
                None,
            )?)
        }
    })?;
    Ok((
        json!({"brep":brep_interchange::definition_record(&brep)?,
        "before":nurbs_curve_definition_value(&source),"after":nurbs_curve_definition_value(&source),
        "distance_certified":false,"full_ribbon_command":false}),
        elapsed,
    ))
}
