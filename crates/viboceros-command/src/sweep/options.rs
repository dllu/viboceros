use super::*;

#[derive(Debug)]
pub(super) struct Options {
    pub(super) rail_name: Option<String>,
    pub(super) parameters: Vec<Real>,
    pub(super) roadlike: Option<bool>,
    pub(super) axis: Option<UnitVector3>,
    pub(super) global: Option<bool>,
    pub(super) refit: Option<bool>,
}

impl Options {
    pub(super) fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let (mut rail_name, mut parameters, mut roadlike, mut axis, mut global) =
            (None, None, None, None, None);
        let mut refit = None;
        for argument in arguments {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "RailName") && rail_name.is_none() && !value.is_empty() {
                rail_name = Some(value);
            } else if option_name_eq(name, "Parameters") && parameters.is_none() {
                parameters = Some(
                    value
                        .split(',')
                        .map(parse_finite_real)
                        .collect::<Result<Vec<_>, _>>()?,
                );
            } else if option_name_eq(name, "FrameStyle") && roadlike.is_none() {
                roadlike = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "freeform" => false,
                        "roadlike" => true,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "Axis") && axis.is_none() {
                axis = Some(
                    Vector3::try_from(parse_single_option_point(value, USAGE)?.to_array())?
                        .normalized_nonzero()?,
                );
            } else if option_name_eq(name, "GlobalShapeBlending") && global.is_none() {
                global = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "RefitRail") && refit.is_none() {
                refit = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let parameters = parameters.ok_or(CommandError::Usage(USAGE))?;
        if axis.is_some() && !roadlike.unwrap_or(false) {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self {
            rail_name: rail_name.map(str::to_owned),
            parameters,
            roadlike,
            axis,
            global,
            refit,
        })
    }
}
