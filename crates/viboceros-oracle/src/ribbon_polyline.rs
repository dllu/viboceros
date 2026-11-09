//! Readonly ordinary polyline ribbons, distinct from the interactive command.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    vertices: Vec<[f64; 3]>,
    #[serde(default)]
    parameters: Option<Vec<f64>>,
    distance: f64,
    #[serde(default)]
    both_sides: bool,
    #[serde(default = "normal")]
    normal: [f64; 3],
    corner: String,
}
fn normal() -> [f64; 3] {
    [0., 0., 1.]
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let points = f
        .vertices
        .iter()
        .copied()
        .map(point)
        .collect::<Result<Vec<_>, _>>()?;
    let source = if let Some(parameters) = &f.parameters {
        Polyline3::try_with_parameters(points, parameters.clone(), tolerance)?
    } else {
        Polyline3::try_new(points, tolerance)?
    };
    let normal = Vector3::try_from(f.normal)?.normalized_nonzero()?;
    let corner = match f.corner.as_str() {
        "Sharp" => viboceros_geometry::RibbonCornerStyle::Sharp,
        "Round" => viboceros_geometry::RibbonCornerStyle::Round,
        "Smooth" => viboceros_geometry::RibbonCornerStyle::Smooth,
        "Chamfer" => viboceros_geometry::RibbonCornerStyle::Chamfer,
        "None" => viboceros_geometry::RibbonCornerStyle::None,
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unsupported polyline ribbon corner",
            ));
        }
    };
    let before = json!({"vertices":f.vertices,"parameters":source.parameters()});
    let (breps, elapsed) = measure_probe(iterations, || -> Result<Vec<Brep>, ProbeError> {
        #[cfg(feature = "native-smlib")]
        {
            Ok(viboceros_geometry::try_polyline_ribbon_with_ruled(
                &source,
                f.distance,
                f.both_sides,
                normal,
                tolerance,
                corner,
                &mut viboceros_smlib::ruled_surface,
            )
            .map_err(CommandError::from)?)
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            Ok(viboceros_geometry::try_polyline_ribbon(
                &source,
                f.distance,
                f.both_sides,
                normal,
                tolerance,
                corner,
            )?)
        }
    })?;
    let definitions = breps
        .iter()
        .map(brep_interchange::definition_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        json!({"breps":definitions,"before":before,"after":before,
        "distance_certified":false,"full_ribbon_command":false}),
        elapsed,
    ))
}
