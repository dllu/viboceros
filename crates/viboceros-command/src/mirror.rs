//! Mirror-plane interpretation, independent of UI picking and document replay.
use super::*;

const USAGE: &str = "Mirror start end | 3Point origin x-point plane-point | XAxis|YAxis|ZAxis | Object object-id [Face=index] [Copy=Yes|No]";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirrorPlaneOption {
    #[default]
    TwoPoint,
    ThreePoint,
    XAxis,
    YAxis,
    ZAxis,
    Object,
}

impl MirrorPlaneOption {
    pub fn from_token(token: &str) -> Option<Self> {
        [
            Self::ThreePoint,
            Self::XAxis,
            Self::YAxis,
            Self::ZAxis,
            Self::Object,
        ]
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
            Self::Object => Some("Object"),
        }
    }
}

/// An unselected, selectable planar surface or B-rep face supplies the plane.
/// Picking it never adds the target to the transform sources. A polysurface
/// requires an explicit face; reflection is independent of face orientation.
pub fn object_plane(
    document: &Document,
    id: ObjectId,
    face: Option<usize>,
) -> Result<viboceros_geometry::Plane, CommandError> {
    let usage =
        || CommandError::Usage("Mirror Object requires a selectable planar surface or face");
    let object = document
        .object(id)
        .filter(|_| document.is_object_selectable(id))
        .ok_or_else(usage)?;
    let surface = match object.geometry() {
        Geometry::NurbsSurface(surface) if face.is_none_or(|face| face == 0) => surface,
        Geometry::Brep(brep) => {
            let index = face
                .or_else(|| (brep.faces().len() == 1).then_some(0))
                .ok_or_else(usage)?;
            brep.faces().get(index).ok_or_else(usage)?.surface()
        }
        _ => return Err(usage()),
    };
    surface.plane(document.tolerance())?.ok_or_else(usage)
}

/// Complete object target grammar, also used by typed interactive picks.
pub fn object_target(arguments: &[&str]) -> Result<(ObjectId, Option<usize>), CommandError> {
    let id = arguments
        .first()
        .ok_or(CommandError::Usage(USAGE))?
        .parse::<ObjectId>()
        .map_err(|_| CommandError::Usage(USAGE))?;
    let face = match &arguments[1..] {
        [] => None,
        [option] => {
            let (name, value) = option.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if !option_name_eq(name, "Face") {
                return Err(CommandError::Usage(USAGE));
            }
            Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            )
        }
        _ => return Err(CommandError::Usage(USAGE)),
    };
    Ok((id, face))
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
        let mut positional = Vec::new();
        let mut copy = None;
        for argument in arguments {
            if let Some((name, value)) = argument.split_once('=')
                && option_name_eq(name, "Copy")
            {
                if copy.is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
                copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else {
                positional.push(*argument);
            }
        }
        let copy = copy.unwrap_or(false);
        let mut source_override = None;
        positional.retain(|argument| {
            if let Some((name, value)) = argument.split_once('=')
                && option_name_eq(name, "Sources")
            {
                if source_override.is_some() {
                    source_override = Some("");
                } else {
                    source_override = Some(value);
                }
                false
            } else {
                true
            }
        });
        let (option, points) = positional
            .first()
            .and_then(|token| MirrorPlaneOption::from_token(token))
            .map_or(
                (MirrorPlaneOption::TwoPoint, positional.as_slice()),
                |option| (option, &positional[1..]),
            );
        let frame = context.construction_plane;
        let selected = if let Some(sources) = source_override {
            if option != MirrorPlaneOption::Object {
                return Err(CommandError::Usage(USAGE));
            }
            let ids = sources
                .split(',')
                .map(|source| {
                    source
                        .parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(USAGE))
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if ids.is_empty()
                || ids.len() != sources.split(',').count()
                || ids.iter().any(|id| !document.is_object_selectable(*id))
            {
                return Err(CommandError::Usage(USAGE));
            }
            document
                .objects()
                .filter_map(|object| ids.contains(&object.id()).then_some(object.id()))
                .collect::<Vec<_>>()
        } else {
            transform_source_ids(document)?
        };
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
            MirrorPlaneOption::Object => {
                let (id, face) = object_target(points)?;
                let plane = object_plane(document, id, face)?;
                (plane.origin(), plane.normal())
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
        if option == MirrorPlaneOption::Object {
            document.clear_selection();
        }
        let (transformed, copied) =
            apply_transform_with_renewal(document, &selected, transform, copy)?;
        if option == MirrorPlaneOption::Object {
            document.clear_selection();
        }
        Ok(format!(
            "Mirrored {transformed} object(s), creating {copied} copy object(s)"
        ))
    }
}
