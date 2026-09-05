use super::*;
use viboceros_geometry::LoftStyle;

const USAGE: &str = "Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No]";

pub(super) struct LoftCommand;

impl Command for LoftCommand {
    fn name(&self) -> &'static str {
        "Loft"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (mut style, mut closed) = (None, None);
        for argument in arguments {
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
            } else {
                return Err(CommandError::Usage(USAGE));
            }
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
        let brep = Brep::try_loft(
            &profiles,
            style.unwrap_or_default(),
            closed.unwrap_or(false),
            document.tolerance(),
        )?;
        let faces = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Lofted {} curve sections into one BRep with {faces} faces",
            profiles.len(),
        ))
    }
}

#[cfg(test)]
mod tests;
