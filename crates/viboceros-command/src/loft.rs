use super::*;
use viboceros_geometry::{LoftEndPoints, LoftOptions, LoftStyle};

const USAGE: &str = "Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No] [SplitAtTangents=Yes|No] [Natural] [StartPoint=x,y,z] [EndPoint=x,y,z]";

pub(super) struct LoftCommand(pub crease_splitting::Preference);

impl Command for LoftCommand {
    fn name(&self) -> &'static str {
        "Loft"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (mut style, mut closed, mut split, mut natural) = (None, None, None, false);
        let mut ends = LoftEndPoints::default();
        for argument in arguments {
            if option_name_eq(argument, "Natural") && !natural {
                natural = true;
                continue;
            }
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Type") && style.is_none() {
                style = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "normal" => LoftStyle::Normal,
                        "loose" => LoftStyle::Loose,
                        "tight" => LoftStyle::Tight,
                        "straight" => LoftStyle::Straight,
                        "uniform" => LoftStyle::Uniform,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "Closed") && closed.is_none() {
                closed = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "SplitAtTangents") && split.is_none() {
                split = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "StartPoint") && ends.start.is_none() {
                ends.start = Some(parse_point(&[value])?.0);
            } else if option_name_eq(name, "EndPoint") && ends.end.is_none() {
                ends.end = Some(parse_point(&[value])?.0);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        if closed.unwrap_or(false) && !ends.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let selected = selected_ids(document)?;
        let profiles = selected
            .iter()
            .map(|id| {
                document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::LoftRequiresCurves)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let options = LoftOptions {
            crease_splitting: self.0.get().0,
            split_at_tangents: split.unwrap_or(false),
            natural_seams: natural,
        };
        let brep = if ends.is_empty() {
            Brep::try_loft_with_surface_options(
                &profiles,
                style.unwrap_or_default(),
                closed.unwrap_or(false),
                options,
                document.tolerance(),
                &mut surface,
            )?
        } else {
            Brep::try_loft_with_surface_end_points(
                &profiles,
                style.unwrap_or_default(),
                ends,
                options,
                document.tolerance(),
                &mut surface_with_end_points,
            )?
        };
        let faces = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Lofted {} curve sections into one BRep with {faces} faces",
            profiles.len(),
        ))
    }
}

fn surface_with_end_points(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface_with_end_points(curves, style, ends)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves_with_end_points(
            curves, style, ends,
        )?)
    }
}

pub(super) fn surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface(curves, style, closed)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves(
            curves, style, closed,
        )?)
    }
}

#[cfg(test)]
mod tests;
