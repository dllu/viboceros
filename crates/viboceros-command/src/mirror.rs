//! Mirror-plane interpretation, independent of UI picking and document replay.
use super::*;

const USAGE: &str =
    "Mirror start end | 3Point origin x-point plane-point | XAxis|YAxis|ZAxis [Copy=Yes|No]";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirrorPlaneOption {
    #[default]
    TwoPoint,
    ThreePoint,
    XAxis,
    YAxis,
    ZAxis,
}

impl MirrorPlaneOption {
    pub fn from_token(token: &str) -> Option<Self> {
        [Self::ThreePoint, Self::XAxis, Self::YAxis, Self::ZAxis]
            .into_iter()
            .find(|option| option_name_eq(token, option.token().unwrap()))
    }

    pub const fn token(self) -> Option<&'static str> {
        match self {
            Self::TwoPoint => None,
            Self::ThreePoint => Some("3Point"),
            Self::XAxis => Some("XAxis"),
            Self::YAxis => Some("YAxis"),
            Self::ZAxis => Some("ZAxis"),
        }
    }
}

/// Options accepted when beginning an interactive Mirror, before any points.
pub fn start_options(
    arguments: &[&str],
    default_copy: bool,
) -> Result<(MirrorPlaneOption, bool), CommandError> {
    let mut plane = None;
    let mut copy = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        if let Some(option) = MirrorPlaneOption::from_token(argument) {
            if plane.replace(option).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            let (name, value) = if let Some(pair) = argument.split_once('=') {
                pair
            } else {
                index += 1;
                (
                    argument,
                    *arguments.get(index).ok_or(CommandError::Usage(USAGE))?,
                )
            };
            if !option_name_eq(name, "Copy") || copy.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        }
        index += 1;
    }
    Ok((plane.unwrap_or_default(), copy.unwrap_or(default_copy)))
}

pub(super) struct MirrorCommand;

impl Command for MirrorCommand {
    fn name(&self) -> &'static str {
        "Mirror"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(true)
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let selected = transform_source_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, USAGE)?;
        let (option, points) = positional
            .first()
            .and_then(|token| MirrorPlaneOption::from_token(token))
            .map_or(
                (MirrorPlaneOption::TwoPoint, positional.as_slice()),
                |option| (option, &positional[1..]),
            );
        let frame = context.construction_plane;
        let (origin, normal) = match option {
            MirrorPlaneOption::TwoPoint => {
                let (start, consumed) = parse_point(points)?;
                let (end, end_consumed) = parse_point(&points[consumed..])?;
                require_consumed(points, consumed + end_consumed, USAGE)?;
                let normal = frame
                    .z_axis()
                    .as_vector()
                    .cross(super::plane_transforms::plane_vector(frame, start, end)?)?
                    .normalized(document.tolerance())?;
                (start, normal)
            }
            MirrorPlaneOption::ThreePoint => {
                let (origin, consumed) = parse_point(points)?;
                let (x, x_consumed) = parse_point(&points[consumed..])?;
                let (y, y_consumed) = parse_point(&points[consumed + x_consumed..])?;
                require_consumed(points, consumed + x_consumed + y_consumed, USAGE)?;
                let plane = Frame3::try_from_points(origin, x, y, document.tolerance())?;
                (origin, plane.z_axis())
            }
            axis => {
                require_consumed(points, 0, USAGE)?;
                (
                    frame.origin(),
                    match axis {
                        MirrorPlaneOption::XAxis => frame.y_axis(),
                        MirrorPlaneOption::YAxis => frame.x_axis(),
                        MirrorPlaneOption::ZAxis => frame.z_axis(),
                        _ => unreachable!(),
                    },
                )
            }
        };
        let transform = AffineTransform3::try_reflection(origin, normal)?;
        let (transformed, copied) =
            apply_transform_with_renewal(document, &selected, transform, copy)?;
        Ok(format!(
            "Mirrored {transformed} object(s), creating {copied} copy object(s)"
        ))
    }
}
