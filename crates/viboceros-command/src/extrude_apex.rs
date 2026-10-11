//! Apex extrusion parsing and transactional document policy.
use super::*;
#[cfg(feature = "native-smlib")]
mod native;

const EXTRUDE_CURVE_TO_POINT_USAGE: &str =
    "ExtrudeCrvToPoint apex [DeleteInput=Yes|No] [Output=Surface] [Solid=Yes|No]";

pub(super) struct ExtrudeCurveToPointCommand;

impl Command for ExtrudeCurveToPointCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvToPoint"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (apex, delete_input, solid) = parse_curve_to_point_extrusion(arguments)?;
        let mut extrusions = Vec::new();
        for id in selected {
            let Some(curve) = document
                .object(id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            #[cfg(not(feature = "native-smlib"))]
            let geometry =
                if solid && curve.is_closed()? && curve.is_planar(document.tolerance())? {
                    Geometry::Brep(Brep::try_extruded_curve_to_point(
                        &curve,
                        apex,
                        document.tolerance(),
                    )?)
                } else {
                    Geometry::NurbsSurface(NurbsSurface::try_extruded_curve_to_point(&curve, apex)?)
                };
            #[cfg(feature = "native-smlib")]
            let geometry = native::extrude(&curve, apex, solid, document.tolerance())?;
            extrusions.push((id, geometry));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }

        let output_count = extrusions.len();
        let solid_count = extrusions
            .iter()
            .filter(|(_, geometry)| matches!(geometry, Geometry::Brep(_)))
            .count();
        for (_, geometry) in &extrusions {
            document.add_geometry(geometry.clone())?;
        }
        if delete_input {
            for (id, _) in &extrusions {
                document.delete_object(*id)?;
            }
        }
        let surface_count = output_count - solid_count;
        Ok(format!(
            "Extruded {output_count} curve object(s) to {solid_count} exact capped B-rep solid(s) and {surface_count} exact NURBS apex surface(s){}",
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_to_point_extrusion(
    arguments: &[&str],
) -> Result<(Point3, bool, bool), CommandError> {
    let mut positional = Vec::new();
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE));
            }
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveToPointExtrusionOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid =
                Some(parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?);
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE));
        }
    }

    let (apex, consumed) = parse_point(&positional)?;
    require_consumed(&positional, consumed, EXTRUDE_CURVE_TO_POINT_USAGE)?;
    Ok((apex, delete_input.unwrap_or(false), solid.unwrap_or(false)))
}
