//! Independent signed X/Y reference scaling in a chosen plane.
use super::*;

pub const USAGE: &str = "ScaleByPlane [Plane=ActiveCPlane|WorldTop|WorldFront|WorldRight|FromView|3Point|Object] [plane-points|object-id [Face=index]] origin reference target [Rigid=Yes|No] [Copy=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaneChoice {
    #[default]
    ActiveCPlane,
    WorldTop,
    WorldFront,
    WorldRight,
    ThreePoint,
    Object,
    FromView,
}

impl PlaneChoice {
    pub fn parse(token: &str) -> Option<Self> {
        [
            Self::ActiveCPlane,
            Self::WorldTop,
            Self::WorldFront,
            Self::WorldRight,
            Self::ThreePoint,
            Self::Object,
            Self::FromView,
        ]
        .into_iter()
        .find(|p| option_name_eq(token, p.token()))
    }

    pub const fn token(self) -> &'static str {
        match self {
            Self::ActiveCPlane => "ActiveCPlane",
            Self::WorldTop => "WorldTop",
            Self::WorldFront => "WorldFront",
            Self::WorldRight => "WorldRight",
            Self::ThreePoint => "3Point",
            Self::Object => "Object",
            Self::FromView => "FromView",
        }
    }

    /// FromView receives the picked viewport's construction plane. Native
    /// Rhino captures distinguish it from that viewport's camera frame.
    pub fn frame(self, active: Frame3) -> Option<Frame3> {
        let world = CommandContext::default().construction_plane;
        match self {
            Self::ActiveCPlane | Self::FromView => Some(active),
            Self::WorldTop => Some(world),
            Self::WorldFront | Self::WorldRight => Some(
                Frame3::try_from_directions(
                    world.origin(),
                    if self == Self::WorldFront {
                        world.x_axis()
                    } else {
                        world.y_axis()
                    }
                    .as_vector(),
                    world.z_axis().as_vector(),
                    Tolerance::DEFAULT,
                )
                .expect("orthogonal world axes"),
            ),
            Self::ThreePoint | Self::Object => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub plane: PlaneChoice,
    pub rigid: bool,
    pub copy: bool,
}

fn options<'a>(
    arguments: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options), CommandError> {
    let mut remaining = Vec::new();
    let mut i = 0;
    while i < arguments.len() {
        let token = arguments[i];
        let (name, value) = token
            .split_once('=')
            .map_or((token, None), |(n, v)| (n, Some(v)));
        if option_name_eq(name, "Plane")
            || option_name_eq(name, "Rigid")
            || option_name_eq(name, "Copy")
        {
            let value = if let Some(value) = value {
                value
            } else {
                i += 1;
                *arguments.get(i).ok_or(CommandError::Usage(USAGE))?
            };
            if option_name_eq(name, "Plane") {
                options.plane = PlaneChoice::parse(value).ok_or(CommandError::Usage(USAGE))?;
            } else {
                let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                if option_name_eq(name, "Rigid") {
                    options.rigid = value;
                } else {
                    options.copy = value;
                }
            }
        } else {
            remaining.push(token);
        }
        i += 1;
    }
    Ok((remaining, options))
}

pub fn start_options(arguments: &[&str], defaults: Options) -> Result<Options, CommandError> {
    let (positional, options) = options(arguments, defaults)?;
    if positional.is_empty() {
        Ok(options)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}

/// Planar curve axes and surface U/V axes determine the independent scales.
/// Resolving a target does not add it to the transform sources.
pub fn object_frame(
    document: &Document,
    id: ObjectId,
    face: Option<usize>,
) -> Result<Frame3, CommandError> {
    let usage =
        || CommandError::Usage("ScaleByPlane Object requires a selectable planar object or face");
    let object = document
        .object(id)
        .filter(|_| document.is_object_selectable(id))
        .ok_or_else(usage)?;
    let tolerance = document.tolerance();
    let frame = match object.geometry() {
        Geometry::NurbsSurface(_) | Geometry::Brep(_) => {
            // Use the public surface planarity predicate before its U/V frame.
            crate::mirror::object_plane(document, id, face).map_err(|_| usage())?;
            match object.geometry() {
                Geometry::Brep(brep) => crate::construction_plane::frame_from_brep_face(
                    brep,
                    face.unwrap_or(0),
                    tolerance,
                ),
                geometry => crate::construction_plane::frame_from_object(geometry, tolerance),
            }
        }
        geometry if face.is_none() && geometry.curve_ref().is_some() => {
            if !geometry.curve_ref().unwrap().is_planar(tolerance)? {
                return Err(usage());
            }
            crate::construction_plane::frame_from_object(geometry, tolerance)
        }
        _ => return Err(usage()),
    }
    .map_err(|_| usage())?;
    Ok(frame)
}

pub fn object_target(arguments: &[&str]) -> Result<(ObjectId, Option<usize>), CommandError> {
    crate::mirror::object_target(arguments).map_err(|_| CommandError::Usage(USAGE))
}

/// Normal components have no effect. An axis whose numerator or denominator
/// has magnitude at most 1e-6 remains unchanged. Native captures include the
/// exact cutoff and its neighboring doubles. Signed ratios allow reflection.
pub fn reference_factors(
    plane: Frame3,
    origin: Point3,
    reference: Point3,
    target: Point3,
) -> Result<[Real; 3], GeometryError> {
    let frame = plane.with_origin(origin);
    let mut factors = [1.; 3];
    for (axis, factor) in factors[..2].iter_mut().enumerate() {
        let denominator = frame.coordinate_of(axis, reference)?;
        let numerator = frame.coordinate_of(axis, target)?;
        if denominator.abs() > 1e-6 && numerator.abs() > 1e-6 {
            *factor = numerator / denominator;
        }
        if !factor.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "ScaleByPlane factor",
            });
        }
    }
    Ok(factors)
}

pub fn scale_map(
    plane: Frame3,
    origin: Point3,
    reference: Point3,
    target: Point3,
) -> Result<AffineTransform3, GeometryError> {
    crate::nonuniform_scale::scale_map(
        plane,
        origin,
        reference_factors(plane, origin, reference, target)?,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prompt {
    pub options: Options,
    pub plane: Option<Frame3>,
    pub plane_points: [Option<Point3>; 2],
    pub plane_menu: bool,
    pub plane_target: Option<(ObjectId, Option<usize>)>,
    pub origin: Option<Point3>,
    pub reference: Option<Point3>,
}

impl Prompt {
    pub fn new(options: Options, active: Frame3) -> Self {
        Self {
            plane: if options.plane == PlaneChoice::FromView {
                None
            } else {
                options.plane.frame(active)
            },
            options,
            plane_points: [None; 2],
            plane_menu: false,
            plane_target: None,
            origin: None,
            reference: None,
        }
    }
    pub const fn prompt(self) -> &'static str {
        if self.plane_menu {
            return "ScaleByPlane: choose ActiveCPlane, 3Point, Object, FromView, WorldTop, WorldFront, or WorldRight";
        }
        if self.plane.is_none() {
            return match self.options.plane {
                PlaneChoice::Object => "ScaleByPlane: pick a planar object or face (Esc to cancel)",
                PlaneChoice::FromView => {
                    "ScaleByPlane: select a viewport (Enter uses active viewport)"
                }
                _ => match self.plane_points {
                    [None, _] => "ScaleByPlane: reference plane origin",
                    [Some(_), None] => "ScaleByPlane: reference plane X direction",
                    _ => "ScaleByPlane: reference plane orientation",
                },
            };
        }
        if self.origin.is_none() {
            "ScaleByPlane: origin (Plane=ActiveCPlane|3Point|Object|FromView|WorldTop|WorldFront|WorldRight, Rigid=Yes|No, Copy=Yes|No)"
        } else if self.reference.is_none() {
            "ScaleByPlane: first reference point (Rigid=Yes|No, Copy=Yes|No)"
        } else {
            "ScaleByPlane: second reference point (Rigid=Yes|No, Copy=Yes|No)"
        }
    }
}

pub(super) struct ScaleByPlaneCommand(remembered::Remembered<bool>);
impl Default for ScaleByPlaneCommand {
    fn default() -> Self {
        Self(remembered::Remembered::new(false))
    }
}
impl Command for ScaleByPlaneCommand {
    fn name(&self) -> &'static str {
        "ScaleByPlane"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn rigid_option_default(&self) -> Option<bool> {
        Some(self.0.get())
    }
    fn remember_rigid_option(&self, value: bool) -> bool {
        self.0.set(value);
        true
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
        let (arguments, sources) = affine_transform_arguments(document, arguments, USAGE)?;
        let (positional, options) = options(
            &arguments,
            Options {
                rigid: self.0.get(),
                ..Options::default()
            },
        )?;
        let mut consumed = 0;
        let plane = match options.plane {
            PlaneChoice::ThreePoint => {
                let (origin, n) = parse_point(&positional)?;
                consumed += n;
                let (x, n) = parse_point(&positional[consumed..])?;
                consumed += n;
                let (y, n) = parse_point(&positional[consumed..])?;
                consumed += n;
                Frame3::try_from_points(origin, x, y, document.tolerance())?
            }
            PlaneChoice::Object => {
                let n = if positional.get(1).is_some_and(|t| {
                    t.split_once('=')
                        .is_some_and(|(n, _)| option_name_eq(n, "Face"))
                }) {
                    2
                } else {
                    1
                };
                if positional.len() < n {
                    return Err(CommandError::Usage(USAGE));
                }
                let (id, face) = object_target(&positional[..n])?;
                consumed += n;
                object_frame(document, id, face)?
            }
            choice => choice.frame(context.construction_plane).unwrap(),
        };
        let (origin, n) = parse_point(&positional[consumed..])?;
        consumed += n;
        let (reference, n) = parse_point(&positional[consumed..])?;
        consumed += n;
        let (target, n) = parse_point(&positional[consumed..])?;
        consumed += n;
        require_consumed(&positional, consumed, USAGE)?;
        let transform = scale_map(plane, origin, reference, target)?;
        if options.plane == PlaneChoice::Object {
            document.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            document.clear_selection();
        }
        let (changed, copied) = if options.rigid {
            crate::rigid_transform::apply(
                document,
                &sources,
                transform,
                AffineTransform3::identity(),
                options.copy,
            )?
        } else {
            apply_transform_with_renewal(document, &sources, transform, options.copy)?
        };
        self.remember_rigid_option(options.rigid);
        if options.plane == PlaneChoice::Object {
            document.clear_selection();
        }
        Ok(format!(
            "Scaled {changed} object(s) by plane, creating {copied} copy object(s)"
        ))
    }
}

#[cfg(test)]
mod tests;
