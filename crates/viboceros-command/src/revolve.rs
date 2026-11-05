//! Revolution input parsing and transactional surface staging.
use super::*;

#[cfg(feature = "native-smlib")]
mod native;
mod options;
mod profile;
pub use options::Options;

const REVOLVE_USAGE: &str = "Revolve axis-start axis-end angle-degrees [StartAngle=degrees] [FullCircle=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Deformable=Yes|No] [PointCount=4..1000] [SplitAtTangents=Yes|No]";

pub(super) struct RevolveCommand;

impl Command for RevolveCommand {
    fn name(&self) -> &'static str {
        "Revolve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (axis_segment, options) = parse_revolution(arguments, document.tolerance())?;
        let axis_origin = axis_segment.start();
        let axis = axis_segment.direction(document.tolerance())?;
        let (start_angle, sweep_angle) = options.angles()?;
        let Options {
            delete_input,
            deformable,
            point_count,
            ..
        } = options;
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
            let curve = if deformable {
                profile::prepare(&curve, axis_segment, document.tolerance())?
            } else {
                curve
            };
            let surface = if deformable {
                NurbsSurface::try_deformable_revolved_curve(
                    &curve,
                    axis_origin,
                    axis,
                    start_angle,
                    sweep_angle,
                    point_count,
                )?
            } else {
                #[cfg(not(feature = "native-smlib"))]
                let exact = NurbsSurface::try_revolved_curve(
                    &curve,
                    axis_origin,
                    axis,
                    start_angle,
                    sweep_angle,
                )?;
                #[cfg(feature = "native-smlib")]
                let exact = native::revolve(&curve, axis_origin, axis, start_angle, sweep_angle)?;
                exact
            };
            let surface = if deformable {
                let mid = curve.evaluate(curve.parameter_at(0.5)?)?;
                let radius =
                    mid.distance_to(axis_segment.closest_point(mid, document.tolerance())?)?;
                let width = radius * sweep_angle.abs();
                let u = if width > 0. {
                    0.0..=width
                } else {
                    surface.domain_u()
                };
                let height = viboceros_geometry::ArcLengthSampler::try_new(
                    CurveRef::NurbsCurve(&curve),
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .quadrature_length_estimate(1e-4)?;
                let v = surface.domain_v();
                let extent = v.end() - v.start();
                // Native uses asymmetric thresholds, preserving well-scaled
                // charts and their possibly nonzero parameter origin.
                let v = if height > 3. * extent || height < 0.33 * extent {
                    0.0..=height
                } else {
                    v
                };
                surface.try_reparameterized(u, v)?
            } else {
                surface
            };
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
            "Revolved {output_count} curve object(s) through {:.6} degrees into {} NURBS surfaces{}",
            sweep_angle.to_degrees(),
            if deformable { "deformable" } else { "exact" },
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
