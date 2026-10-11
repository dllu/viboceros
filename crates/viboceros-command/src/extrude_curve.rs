//! Curve extrusion parsing, geometry staging, and document policy.
use super::*;

#[cfg(feature = "native-smlib")]
mod native;

const EXTRUDE_CURVE_USAGE: &str = "ExtrudeCrv distance | base target [BothSides=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Solid=Yes|No]";

pub(super) struct ExtrudeCurveCommand;

impl Command for ExtrudeCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (start_offset, end_offset, delete_input, both_sides, solid) =
            parse_curve_extrusion(arguments, document.tolerance())?;
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
                    Geometry::Brep(Brep::try_extruded_curve(
                        &curve,
                        start_offset,
                        end_offset,
                        document.tolerance(),
                    )?)
                } else {
                    Geometry::NurbsSurface(NurbsSurface::try_extruded_curve(
                        &curve,
                        start_offset,
                        end_offset,
                    )?)
                };
            #[cfg(feature = "native-smlib")]
            let geometry = native::extrude(
                &curve,
                start_offset,
                end_offset,
                solid,
                document.tolerance(),
            )?;
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
            "Extruded {output_count} curve object(s) into {solid_count} exact capped B-rep solid(s) and {surface_count} exact NURBS surface(s){}{}",
            if both_sides { " on both sides" } else { "" },
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_extrusion(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<(Vector3, Vector3, bool, bool, bool), CommandError> {
    let mut positional = Vec::new();
    let mut both_sides = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "BothSides") && both_sides.is_none() {
            both_sides = parse_yes_no(value);
            if both_sides.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveExtrusionOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid = Some(parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?);
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
        }
    }

    let direction = if positional.len() == 1 && !positional[0].contains(',') {
        Vector3::try_new(0.0, 0.0, parse_finite_real(positional[0])?)?
    } else {
        let (base, base_consumed) = parse_point(&positional)?;
        let (target, target_consumed) = parse_point(&positional[base_consumed..])?;
        require_consumed(
            &positional,
            base_consumed + target_consumed,
            EXTRUDE_CURVE_USAGE,
        )?;
        base.vector_to(target)?
    };
    direction.normalized(tolerance)?;
    let both_sides = both_sides.unwrap_or(false);
    let start_offset = if both_sides {
        direction.scaled(-1.0)?
    } else {
        Vector3::try_new(0.0, 0.0, 0.0)?
    };
    Ok((
        start_offset,
        direction,
        delete_input.unwrap_or(false),
        both_sides,
        solid.unwrap_or(false),
    ))
}
