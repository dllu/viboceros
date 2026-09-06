//! Group-aware one-dimensional distribution with staged, identity-preserving edits.
use crate::{
    Command, CommandContext, CommandError, option_name_eq, parse_finite_real, parse_point,
};
use std::collections::BTreeMap;
use viboceros_document::{Document, GroupId, ObjectId};
use viboceros_geometry::{AffineTransform3, Frame3, GeometryError, Point3, Tolerance, Vector3};
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
enum Direction {
    Axis(usize),
    Points(Point3, Point3),
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

impl DistributionSettings {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
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
            mode: mode.unwrap_or(Mode::Gap),
            spacing: spacing.unwrap_or(None),
        })
    }
}

pub(super) struct DistributeCommand;

/// Number of independently distributed units in the current selection.
pub fn distribution_unit_count(document: &Document) -> usize {
    units(
        document,
        &document.selected_object_ids().collect::<Vec<_>>(),
    )
    .len()
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
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let units = units(document, &selected);
        if units.len() < 3 {
            return Err(CommandError::InsufficientDistributionObjects {
                actual: units.len(),
            });
        }
        let anchor = document
            .object(selected[0])
            .unwrap()
            .geometry()
            .bounds()
            .center()?;
        let frame = direction_frame(
            options.direction,
            context.construction_plane,
            anchor,
            document.tolerance(),
        )?;
        let bounds = units
            .iter()
            .map(|ids| {
                crate::object_bounds::local_bounds(
                    ids.iter()
                        .map(|id| document.object(*id).unwrap().geometry()),
                    frame,
                    document.tolerance(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut order = (0..units.len()).collect::<Vec<_>>();
        order.sort_by(|a, b| {
            bounds[*a]
                .min()
                .to_array()
                .partial_cmp(&bounds[*b].min().to_array())
                .expect("finite bounds")
        });
        let intervals = order
            .iter()
            .map(|i| [bounds[*i].min().x(), bounds[*i].max().x()])
            .collect::<Vec<_>>();
        let (offsets, spacing) =
            offsets(&intervals, options.settings.mode, options.settings.spacing)?;
        let mut replacements = Vec::new();
        for (index, distance) in order.into_iter().zip(offsets) {
            if distance == 0. {
                continue;
            }
            let translation = frame.x_axis().as_vector().scaled(distance)?;
            let transform = AffineTransform3::from_translation(translation);
            for id in &units[index] {
                let geometry = document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .transformed(transform, document.tolerance())?;
                replacements.push((*id, geometry));
            }
        }
        let changed = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Distributed {} object/group unit(s) with {:?} spacing {spacing:.6}; moved {changed} object(s)",
            units.len(),
            options.settings.mode
        ))
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut direction = None;
    let mut settings = Vec::new();
    let mut remaining = arguments;
    while let Some(token) = remaining.first() {
        if let Some(axis) = ["XAxis", "YAxis", "ZAxis"]
            .iter()
            .position(|name| option_name_eq(token, name))
        {
            if direction.replace(Direction::Axis(axis)).is_some() {
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
        Direction::Axis(1) => (axes[1].as_vector(), axes[0].as_vector().scaled(-1.)?),
        Direction::Axis(axis) => (axes[axis].as_vector(), axes[(axis + 1) % 3].as_vector()),
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

/// Each object's last membership supplies its rigid unit. Partial selection
/// never moves unseen members, even when that top group contains them.
fn units(document: &Document, selected: &[ObjectId]) -> Vec<Vec<ObjectId>> {
    #[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
    enum Unit {
        Object(ObjectId),
        Group(GroupId),
    }
    // Join selection order to document order in one pass; avoid a linear
    // document lookup for every selected object in large distributions.
    let indices = selected
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect::<BTreeMap<_, _>>();
    let mut memberships = vec![None; selected.len()];
    for object in document.objects() {
        if let Some(index) = indices.get(&object.id()) {
            memberships[*index] = object.top_group();
        }
    }
    let mut slots = BTreeMap::new();
    let mut result = Vec::<Vec<ObjectId>>::new();
    for (i, id) in selected.iter().copied().enumerate() {
        let key = memberships[i].map_or(Unit::Object(id), Unit::Group);
        let slot = *slots.entry(key).or_insert_with(|| {
            result.push(Vec::new());
            result.len() - 1
        });
        result[slot].push(id);
    }
    result
}

fn offsets(
    intervals: &[[f64; 2]],
    mode: Mode,
    explicit: Option<f64>,
) -> Result<(Vec<f64>, f64), GeometryError> {
    let center = |[a, b]: [f64; 2]| 0.5 * a + 0.5 * b;
    let count = intervals.len();
    let divisor = (count - 1) as f64;
    let spacing = explicit.unwrap_or_else(|| match mode {
        Mode::Center => (center(intervals[count - 1]) - center(intervals[0])) / divisor,
        Mode::Gap => {
            // Average existing gaps avoids subtracting two large total widths.
            let mut sum = 0.;
            let mut correction = 0.;
            for pair in intervals.windows(2) {
                let gap = (pair[1][0] - pair[0][1]) / divisor;
                let adjusted = gap - correction;
                let next = sum + adjusted;
                correction = (next - sum) - adjusted;
                sum = next;
            }
            sum
        }
    });
    let mut offsets = vec![0.; count];
    let mut next_min = intervals[0][1] + spacing;
    for i in 1..count {
        offsets[i] = match mode {
            Mode::Center => spacing.mul_add(i as f64, center(intervals[0])) - center(intervals[i]),
            Mode::Gap => next_min - intervals[i][0],
        };
        next_min += intervals[i][1] - intervals[i][0] + spacing;
    }
    if explicit.is_none() {
        offsets[count - 1] = 0.;
    }
    // Reuse the validated scalar/vector boundary; no nonfinite displacement
    // is allowed to reach document geometry or to be hidden by a no-op.
    Vector3::try_new(spacing, 0., 0.)?;
    for value in &offsets {
        Vector3::try_new(*value, 0., 0.)?;
    }
    Ok((offsets, spacing))
}
