//! Group-aware one-dimensional distribution with staged, identity-preserving edits.
use crate::{
    Command, CommandContext, CommandError, option_name_eq, parse_finite_real, parse_point,
};
mod layout;
mod planner;
pub use layout::{Layout, Sources};
use viboceros_document::Document;
use viboceros_geometry::{Frame3, GeometryError, Point3, Tolerance};
#[cfg(test)]
mod tests;

pub(super) const USAGE: &str =
    "Distribute XAxis|YAxis|ZAxis|Direction from to [Mode=Gap|Center] [Spacing=Automatic|distance]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistributionMode {
    Gap,
    Center,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    XAxis,
    YAxis,
    ZAxis,
    Points(Point3, Point3),
}

impl Direction {
    pub fn axis(token: &str) -> Option<Self> {
        [
            (Self::XAxis, "XAxis"),
            (Self::YAxis, "YAxis"),
            (Self::ZAxis, "ZAxis"),
        ]
        .into_iter()
        .find_map(|(axis, name)| option_name_eq(token, name).then_some(axis))
    }
}
struct Options {
    direction: Direction,
    settings: DistributionSettings,
}

use DistributionMode as Mode;

/// Shared command/UI settings, independent of direction-point collection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistributionSettings {
    pub mode: DistributionMode,
    pub spacing: Option<f64>,
}

impl Default for DistributionSettings {
    fn default() -> Self {
        Self {
            mode: Mode::Gap,
            spacing: None,
        }
    }
}

impl DistributionSettings {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        Self::parse_over(arguments, Self::default())
    }

    pub fn parse_over(arguments: &[&str], defaults: Self) -> Result<Self, CommandError> {
        let mut mode = None;
        let mut spacing = None;
        let mut remaining = arguments;
        while let Some(token) = remaining.first() {
            let (name, value, used) = if let Some((name, value)) = token.split_once('=') {
                (name, value, 1)
            } else {
                (
                    *token,
                    *remaining.get(1).ok_or(CommandError::Usage(USAGE))?,
                    2,
                )
            };
            let value = value.trim_start_matches('_');
            if option_name_eq(name, "Mode") && mode.is_none() {
                mode = Some(if value.eq_ignore_ascii_case("Gap") {
                    Mode::Gap
                } else if value.eq_ignore_ascii_case("Center") {
                    Mode::Center
                } else {
                    return Err(CommandError::Usage(USAGE));
                });
            } else if option_name_eq(name, "Spacing") && spacing.is_none() {
                spacing = Some(if value.eq_ignore_ascii_case("Automatic") {
                    None
                } else {
                    Some(parse_finite_real(value)?)
                });
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            remaining = &remaining[used..];
        }
        Ok(Self {
            mode: mode.unwrap_or(defaults.mode),
            spacing: spacing.unwrap_or(defaults.spacing),
        })
    }
}

pub(super) struct DistributeCommand;

/// Number of independently distributed units in the current selection.
pub fn distribution_unit_count(document: &Document) -> usize {
    Sources::capture(document, &[]).unit_count()
}

impl Command for DistributeCommand {
    fn name(&self) -> &'static str {
        "Distribute"
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
        let options = parse(arguments)?;
        Sources::capture(document, &[])
            .layout(
                options.direction,
                context.construction_plane,
                options.settings,
            )?
            .apply(document, false)
    }
}

pub fn parse_positional(
    arguments: &[&str],
) -> Result<(Direction, DistributionSettings), CommandError> {
    parse(arguments).map(|options| (options.direction, options.settings))
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut direction = None;
    let mut settings = Vec::new();
    let mut remaining = arguments;
    while let Some(token) = remaining.first() {
        if let Some(axis) = Direction::axis(token) {
            if direction.replace(axis).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            remaining = &remaining[1..];
        } else if option_name_eq(token, "Direction") {
            if direction.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (a, n) = parse_point(&remaining[1..])?;
            let (b, m) = parse_point(&remaining[1 + n..])?;
            direction = Some(Direction::Points(a, b));
            remaining = &remaining[1 + n + m..];
        } else {
            let used = if token.contains('=') { 1 } else { 2 };
            settings.extend_from_slice(remaining.get(..used).ok_or(CommandError::Usage(USAGE))?);
            remaining = &remaining[used..];
        }
    }
    Ok(Options {
        direction: direction.ok_or(CommandError::Usage(USAGE))?,
        settings: DistributionSettings::parse(&settings)?,
    })
}

fn direction_frame(
    direction: Direction,
    plane: Frame3,
    origin: Point3,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let axes = plane.axes();
    let (direction, guide) = match direction {
        // Rotate the CPlane XY axes a quarter-turn for Y distribution. The
        // transverse coordinates are observable tie-breakers, not arbitrary.
        Direction::XAxis => (axes[0].as_vector(), axes[1].as_vector()),
        Direction::YAxis => (axes[1].as_vector(), axes[0].as_vector().scaled(-1.)?),
        Direction::ZAxis => (axes[2].as_vector(), axes[0].as_vector()),
        Direction::Points(a, b) => {
            let direction = a.vector_to(b)?.normalized(tolerance)?;
            // Preserve the CPlane normal when defining the transverse guide.
            // For a normal-aligned direction, CPlane Y supplies the normal
            // instead. This also gives the observed signed-axis tie ordering.
            let mut guide = axes[2].as_vector().cross(direction.as_vector())?;
            if guide.length()? <= tolerance.angular() {
                guide = axes[1].as_vector().cross(direction.as_vector())?;
            }
            (
                direction.as_vector(),
                guide.normalized_nonzero()?.as_vector(),
            )
        }
    };
    Frame3::try_from_directions(origin, direction, guide, tolerance)
}
