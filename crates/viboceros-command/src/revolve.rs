//! Revolution input parsing and transactional surface staging.
use super::*;

#[cfg(feature = "native-smlib")]
mod native;

const REVOLVE_USAGE: &str = "Revolve axis-start axis-end angle-degrees [StartAngle=degrees] [FullCircle=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Deformable=No] [SplitAtTangents=Yes|No]";

pub(super) struct RevolveCommand;

impl Command for RevolveCommand {
    fn name(&self) -> &'static str {
        "Revolve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (axis_origin, axis, start_angle, sweep_angle, delete_input) =
            parse_revolution(arguments, document.tolerance())?;
        let mut revolutions = Vec::new();
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
            let surface = NurbsSurface::try_revolved_curve(
                &curve,
                axis_origin,
                axis,
                start_angle,
                sweep_angle,
            )?;
            #[cfg(feature = "native-smlib")]
            let surface = native::revolve(&curve, axis_origin, axis, start_angle, sweep_angle)?;
            revolutions.push((id, surface));
        }
        if revolutions.is_empty() {
            return Err(CommandError::NoRevolvableCurves);
        }

        let output_count = revolutions.len();
        for (_, surface) in &revolutions {
            document.add_geometry(Geometry::NurbsSurface(surface.clone()))?;
        }
        if delete_input {
            for (id, _) in revolutions {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Revolved {output_count} curve object(s) through {:.6} degrees into exact NURBS surfaces{}",
            sweep_angle.to_degrees(),
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_revolution(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<(Point3, UnitVector3, Real, Real, bool), CommandError> {
    let mut positional = Vec::new();
    let mut start_angle_degrees = None;
    let mut full_circle = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut deformable_seen = false;
    let mut split_at_tangents = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "StartAngle") && start_angle_degrees.is_none() {
            start_angle_degrees = Some(parse_finite_real(value)?);
        } else if option_name_eq(name, "FullCircle") && full_circle.is_none() {
            full_circle = Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedRevolveOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Deformable") && !deformable_seen {
            let deformable = parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?;
            if deformable {
                return Err(CommandError::DeformableRevolveUnsupported);
            }
            deformable_seen = true;
        } else if option_name_eq(name, "SplitAtTangents") && split_at_tangents.is_none() {
            split_at_tangents =
                Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        }
    }

    let (axis_origin, origin_consumed) = parse_point(&positional)?;
    let (axis_end, end_consumed) = parse_point(&positional[origin_consumed..])?;
    let remaining = &positional[origin_consumed + end_consumed..];
    let sweep_degrees = if full_circle.unwrap_or(false) {
        if !remaining.is_empty() {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        }
        360.0
    } else {
        let [angle] = remaining else {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        };
        let degrees = parse_finite_real(angle)?;
        if degrees == 0.0 || degrees.abs() > 360.0 {
            return Err(CommandError::InvalidRevolutionAngle((*angle).to_owned()));
        }
        degrees
    };
    let start_angle = start_angle_degrees.unwrap_or(0.0).to_radians();
    let sweep_angle = if sweep_degrees.abs() == 360.0 {
        sweep_degrees.signum() * std::f64::consts::TAU
    } else {
        sweep_degrees.to_radians()
    };
    if !start_angle.is_finite() || !sweep_angle.is_finite() {
        return Err(CommandError::InvalidRevolutionAngle(
            sweep_degrees.to_string(),
        ));
    }
    let axis = axis_origin.vector_to(axis_end)?.normalized(tolerance)?;
    Ok((
        axis_origin,
        axis,
        start_angle,
        sweep_angle,
        delete_input.unwrap_or(false),
    ))
}
