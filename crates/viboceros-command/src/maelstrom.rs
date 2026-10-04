//! Cylindrical Maelstrom deformation and atomic source replacement.
use super::*;
use std::collections::BTreeMap;
use viboceros_document::{CopyGroupPolicy, ObjectId, ReplacementHistory};
use viboceros_geometry::{BoundingBox3, MaelstromPointMorph};

const SDK_ZERO: Real = 2.3283064365386963e-10;

pub const USAGE: &str =
    "Maelstrom center first-radius second-radius coil-angle [Copy=Yes|No] [Rigid=Yes|No]";
// Retained actual Line/Curve/Surface definitions are identical at 1e-5 and 1e-9.
const MINIMUM_FITTING_TOLERANCE: Real = 1e-5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MaelstromOptions {
    pub copy: bool,
    pub rigid: bool,
}

impl MaelstromOptions {
    pub fn update(&mut self, input: &str) -> Result<(), CommandError> {
        let tokens = input.split_whitespace().collect::<Vec<_>>();
        let (name, value) = match tokens.as_slice() {
            [token] => token.split_once('=').ok_or(CommandError::Usage(USAGE))?,
            [name, value] => (*name, *value),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        let target = if option_name_eq(name, "Copy") {
            &mut self.copy
        } else if option_name_eq(name, "Rigid") {
            &mut self.rigid
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        *target = value;
        Ok(())
    }
    pub fn command_options(self) -> String {
        let yes_no = |v| if v { "Yes" } else { "No" };
        format!("Copy={} Rigid={}", yes_no(self.copy), yes_no(self.rigid))
    }
    pub fn from_arguments<'a>(
        arguments: &[&'a str],
        default: Self,
    ) -> Result<(Vec<&'a str>, Self), CommandError> {
        let mut options = default;
        let mut positional = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for token in arguments {
            if let Some((name, _)) = token.split_once('=') {
                if !seen.insert(name.trim_start_matches(['_', '-']).to_ascii_lowercase()) {
                    return Err(CommandError::Usage(USAGE));
                }
                options.update(token)?;
            } else {
                positional.push(*token);
            }
        }
        Ok((positional, options))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaelstromRadius {
    Number(Real),
    Point(Point3),
}
impl MaelstromRadius {
    /// Comma coordinates are world points measured from the Maelstrom center.
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        if input.contains(',') {
            let (point, n) = parse_point(&[input])?;
            require_consumed(&[input], n, USAGE)?;
            Ok(Self::Point(point))
        } else {
            Ok(Self::Number(parse_finite_real(input)?))
        }
    }
    pub fn command_argument(self) -> String {
        match self {
            Self::Number(v) => v.to_string(),
            Self::Point(p) => p.to_array().map(|v| v.to_string()).join(","),
        }
    }
    pub fn radius(self, frame: Frame3) -> Result<Real, GeometryError> {
        match self {
            Self::Number(value) => Ok(value),
            Self::Point(p) => frame.origin().distance_to(p),
        }
    }
    fn target_radius(self, frame: Frame3) -> Result<Real, GeometryError> {
        match self {
            Self::Number(value) => Ok(value.abs()),
            Self::Point(p) => {
                let [x, y] = frame.projected_coordinates_of(p)?;
                let radius = x.hypot(y);
                if radius <= SDK_ZERO {
                    return Err(GeometryError::Degenerate {
                        context: "maelstrom second radius pick",
                    });
                }
                Ok(radius)
            }
        }
    }
}

/// Numeric first radii retain the CPlane. A picked first radius follows Circle's
/// x-direction/preferred-normal rule, including its parallel-direction fallback.
pub fn circle_frame(
    center: Point3,
    initial: MaelstromRadius,
    context: CommandContext,
) -> Result<Frame3, GeometryError> {
    let plane = context.construction_plane;
    match initial {
        MaelstromRadius::Number(_) => Frame3::try_from_directions(
            center,
            plane.x_axis().as_vector(),
            plane.y_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        ),
        MaelstromRadius::Point(point) => {
            let direction = center.vector_to(point)?.normalized_nonzero()?.as_vector();
            Frame3::try_from_x_and_normal(
                center,
                direction,
                plane.z_axis().as_vector(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .or_else(|_| {
                Frame3::try_from_directions(
                    center,
                    direction,
                    plane.y_axis().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
            })
        }
    }
}

/// Rotation follows the normal of the resolved first Circle.
pub fn point_morph(
    center: Point3,
    initial: MaelstromRadius,
    target: MaelstromRadius,
    degrees: Real,
    context: CommandContext,
) -> Result<MaelstromPointMorph, GeometryError> {
    let frame = circle_frame(center, initial, context)?;
    let radius0 = initial.radius(frame)?;
    let radius1 = target.target_radius(frame)?;
    // The first radius uses Circle's positive size getter. The second accepts
    // signed distances by magnitude; a zero/tiny SDK radius yields an unchanged
    // point map instead of rejecting the command.
    if !radius1.is_finite() || !degrees.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "maelstrom command",
        });
    }
    if radius1 <= SDK_ZERO {
        MaelstromPointMorph::try_new(frame, radius0, radius0, 0.)
    } else {
        MaelstromPointMorph::try_new(frame, radius0, radius1, degrees.to_radians())
    }
}

/// Stage all sources before changing geometry, selection, attributes or groups.
#[allow(clippy::too_many_arguments)]
pub fn deformed_geometries(
    document: &Document,
    ids: &[ObjectId],
    center: Point3,
    initial: MaelstromRadius,
    target: MaelstromRadius,
    degrees: Real,
    options: MaelstromOptions,
    context: CommandContext,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let morph = point_morph(center, initial, target, degrees, context)?;
    let fitting_tolerance = Tolerance::try_new(
        document
            .tolerance()
            .absolute()
            .max(MINIMUM_FITTING_TOLERANCE),
        document.tolerance().relative(),
        document.tolerance().angular(),
    )?;
    let mut group_bounds = BTreeMap::new();
    if options.rigid && !morph.is_identity() {
        for id in ids {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            if let Some(group) = object.top_group() {
                let bounds = object.geometry().tight_bounds(document.tolerance())?;
                if let Some(b) = group_bounds.get_mut(&group) {
                    *b = BoundingBox3::union(*b, bounds)?;
                } else {
                    group_bounds.insert(group, bounds);
                }
            }
        }
    }
    let transforms = group_bounds
        .into_iter()
        .map(|(id, bounds)| Ok((id, morph.rigid_transform(bounds.center()?)?)))
        .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
    ids.iter()
        .map(|id| {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if morph.is_identity() {
                if matches!(object.geometry(), Geometry::Line(_)) {
                    Geometry::NurbsCurve(
                        object
                            .geometry()
                            .nurbs_curve_representation()?
                            .expect("line NURBS representation"),
                    )
                } else {
                    object.geometry().clone()
                }
            } else if options.rigid {
                let transform = if let Some(group) = object.top_group() {
                    transforms[&group]
                } else {
                    morph.rigid_transform(
                        object
                            .geometry()
                            .tight_bounds(document.tolerance())?
                            .center()?,
                    )?
                };
                object
                    .geometry()
                    .transformed(transform, document.tolerance())?
            } else {
                let tolerance = if matches!(
                    object.geometry(),
                    Geometry::Point(_) | Geometry::PointCloud(_) | Geometry::Mesh(_)
                ) {
                    document.tolerance()
                } else {
                    fitting_tolerance
                };
                object.geometry().morphed(&morph, tolerance)?
            };
            Ok((*id, geometry))
        })
        .collect()
}

pub(super) struct MaelstromPreferences {
    radius: remembered::Remembered<Real>,
}
impl Default for MaelstromPreferences {
    fn default() -> Self {
        Self {
            radius: remembered::Remembered::new(1.),
        }
    }
}
impl CommandRegistry {
    pub fn maelstrom_options_default(&self) -> MaelstromOptions {
        MaelstromOptions {
            copy: self.copy_default("Maelstrom").unwrap_or(false),
            rigid: false,
        }
    }
    pub fn maelstrom_radius_default(&self) -> Real {
        self.maelstrom_preferences.radius.get()
    }
    /// The first Circle radius saves when accepted, including a later Cancel.
    pub fn remember_maelstrom_radius(&self, radius: Real) -> bool {
        if !radius.is_finite() || radius <= SDK_ZERO {
            return false;
        }
        self.maelstrom_preferences.radius.set(radius);
        true
    }
    /// Maelstrom saves Copy getter edits immediately, including cancellation.
    pub fn remember_maelstrom_copy_option(&self, copy: bool) {
        self.copy_preferences.complete("Maelstrom", copy);
    }
}
pub(super) struct MaelstromCommand(
    pub(super) std::sync::Arc<MaelstromPreferences>,
    pub(super) copy_options::CopyPreferences,
);
impl Command for MaelstromCommand {
    fn name(&self) -> &'static str {
        "Maelstrom"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
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
        let (arguments, sources) = transform_arguments(document, arguments, USAGE)?;
        let (positional, options) =
            MaelstromOptions::from_arguments(&arguments, MaelstromOptions::default())?;
        let (center, n) = parse_point(&positional)?;
        require_consumed(&positional, n + 3, USAGE)?;
        let initial = MaelstromRadius::parse(positional[n])?;
        let frame = circle_frame(center, initial, context)?;
        let first_radius = initial.radius(frame)?;
        if first_radius <= SDK_ZERO {
            return Err(GeometryError::Degenerate {
                context: "maelstrom first radius",
            }
            .into());
        }
        // Getter choices are application preferences, independent of model
        // rollback. They save after the first Circle radius is accepted.
        self.0.radius.set(first_radius);
        self.1.complete("Maelstrom", options.copy);
        let target = MaelstromRadius::parse(positional[n + 1])?;
        let degrees = parse_finite_real(positional[n + 2])?;
        let staged = deformed_geometries(
            document,
            &sources.ids,
            center,
            initial,
            target,
            degrees,
            options,
            context,
        )?;
        if sources.release_on_replay {
            document.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
        }
        let count = staged.len();
        if options.copy {
            document.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
            document.select_command_results(sources.ids.iter().copied())?;
        } else {
            document.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            document.move_objects_to_end_in_order(sources.ids.iter().copied())?;
            if sources.postselected {
                document.clear_selection();
            }
        }

        Ok(format!(
            "Morphed {count} object(s){}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}
#[cfg(test)]
mod tests;
