//! Revolution input parsing and transactional surface staging.
use super::*;

mod construction;
#[cfg(feature = "native-smlib")]
mod native;
mod options;
mod profile;
pub use options::{Options, Output};

const REVOLVE_USAGE: &str = "Revolve axis-start axis-end angle-degrees [StartAngle=degrees] [FullCircle=Yes|No] [DeleteInput=Yes|No] [Output=Surface|SubD] [SegmentCount=4..100000] [Deformable=Yes|No] [PointCount=4..1000] [SplitAtTangents=Yes|No]";

pub(super) struct RevolveCommand(pub crease_splitting::Preference);

impl Command for RevolveCommand {
    fn name(&self) -> &'static str {
        "Revolve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (axis_segment, options) = parse_revolution(arguments, document.tolerance())?;
        let (_, sweep_angle) = options.angles()?;
        let Options {
            delete_input,
            deformable,
            ..
        } = options;
        let crease = self.0.get().0;
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
            let geometry = construction::construct(
                &curve,
                axis_segment,
                options,
                crease,
                document.tolerance(),
            )?;
            revolutions.push((id, geometry));
        }
        if revolutions.is_empty() {
            return Err(CommandError::NoRevolvableCurves);
        }

        let output_count = revolutions.len();
        for (_, geometry) in &revolutions {
            document.add_geometry(geometry.clone())?;
        }
        if delete_input {
            for (id, _) in revolutions {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Revolved {output_count} curve object(s) through {:.6} degrees ({}){}",
            sweep_angle.to_degrees(),
            if options.output == Output::SubD {
                "SubD from fitted profiles"
            } else if deformable {
                "deformable"
            } else {
                "exact"
            },
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
) -> Result<(LineSegment, Options), CommandError> {
    let (options, positional): (Vec<_>, Vec<_>) = arguments
        .iter()
        .copied()
        .partition(|a| a.contains('=') || option_name_eq(a, "FullCircle"));
    let mut parsed = Options::default().updated(&options)?;
    let (origin, consumed) = parse_point(&positional)?;
    let (end, end_consumed) = parse_point(&positional[consumed..])?;
    let remaining = &positional[consumed + end_consumed..];
    let option_sweep = options.iter().any(|a| {
        let (name, value) = a.split_once('=').unwrap_or((a, "Yes"));
        option_name_eq(name, "Angle")
            || option_name_eq(name, "SweepAngle")
            || option_name_eq(name, "FullCircle") && parse_yes_no(value) == Some(true)
    });
    match remaining {
        [angle] if !option_sweep => parsed.sweep_degrees = parse_finite_real(angle)?,
        [] if option_sweep => {}
        _ => return Err(CommandError::Usage(REVOLVE_USAGE)),
    }
    parsed.angles()?;
    Ok((LineSegment::try_new(origin, end, tolerance)?, parsed))
}
