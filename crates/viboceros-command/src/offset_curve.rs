//! Analytic, smooth, and planar polyline offsets with corner and layer choices.

use super::*;
use viboceros_geometry::{RibbonCornerStyle, UnitVector3};
mod getter;
mod multiple;
mod staged;
pub use getter::{Cap, Kind, OptionName, Settings, parse_length, parse_settings};
use multiple::MultipleOptions;
pub use staged::{Draft, Plan, SourceOwner, Target};

#[cfg(test)]
mod multiple_tests;
#[cfg(test)]
mod tests;

const USAGE: &str = "Offset distance side-point [BothSides=Yes|No] [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input] | Offset distance BothSides=Yes [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input] | Offset ThroughPoint=point [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input]";

pub(super) fn offset_operations() -> Option<viboceros_geometry::CurveOffsetOperations> {
    #[cfg(feature = "native-smlib")]
    {
        Some(viboceros_smlib::curve_offset_operations())
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        None
    }
}

pub(super) struct OffsetCommand(pub std::sync::Arc<remembered::Remembered<Settings>>);
pub(super) struct OffsetMultipleCommand(pub std::sync::Arc<remembered::Remembered<Settings>>);

const MULTIPLE_USAGE: &str = "OffsetMultiple distance side-point [OffsetCount=2] [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input]";
const MAX_OFFSET_OUTPUTS: usize = 100_000;

impl Command for OffsetMultipleCommand {
    fn name(&self) -> &'static str {
        "OffsetMultiple"
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if parse_multiple(args).is_ok() {
            return Ok(None);
        }
        staged::selection_prompt(Kind::Multiple, args)
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
        let options = parse_multiple(arguments)?;
        let sources = selected_curves(document)?;
        let curves = multiple::solve(
            &sources,
            options,
            context.construction_plane.z_axis(),
            document.tolerance(),
            false,
        )?;
        let count = admit(document, curves, options.input_layer)?;
        self.0.set(Settings {
            distance: options.distance,
            count: options.count,
            corner: options.corner,
            input_layer: options.input_layer,
            ..self.0.get()
        });
        Ok(format!("Created {count} offset curve(s)"))
    }
}

pub fn is_positional(kind: Kind, args: &[&str]) -> bool {
    match kind {
        Kind::Offset => parse(args).is_ok(),
        Kind::Multiple => parse_multiple(args).is_ok(),
    }
}

fn parse_multiple(arguments: &[&str]) -> Result<MultipleOptions, CommandError> {
    let distance = parse_finite_real(
        arguments
            .first()
            .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
    )?;
    if distance <= 0.0 {
        return Err(GeometryError::InvalidCurveOffsetDistance.into());
    }
    let (side, consumed) = parse_point(
        arguments
            .get(1..)
            .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
    )?;
    let mut options = MultipleOptions {
        distance,
        side,
        count: 2,
        loose: false,
        corner: RibbonCornerStyle::Sharp,
        input_layer: false,
    };
    let mut seen = [false; 3];
    let mut index = 1 + consumed;
    while index < arguments.len() {
        let (name, value, consumed) = if let Some((name, value)) = arguments[index].split_once('=')
        {
            (name, value, 1)
        } else {
            (
                arguments[index],
                *arguments
                    .get(index + 1)
                    .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
                2,
            )
        };
        let slot = if name.eq_ignore_ascii_case("OffsetCount") {
            0
        } else if name.eq_ignore_ascii_case("Corner") {
            1
        } else if name.eq_ignore_ascii_case("OutputLayer") {
            2
        } else {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        };
        if seen[slot] {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        }
        seen[slot] = true;
        match slot {
            0 => {
                options.count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0 && n <= MAX_OFFSET_OUTPUTS)
                    .ok_or(CommandError::Usage(MULTIPLE_USAGE))?
            }
            1 => {
                options.corner = getter::corner(value)
                    .ok_or(CommandError::Usage(MULTIPLE_USAGE))?
                    .into()
            }
            _ => {
                options.input_layer =
                    getter::layer(value).ok_or(CommandError::Usage(MULTIPLE_USAGE))?
            }
        }
        index += consumed;
    }
    Ok(options)
}

impl Command for OffsetCommand {
    fn name(&self) -> &'static str {
        "Offset"
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if parse(args).is_ok() {
            return Ok(None);
        }
        staged::selection_prompt(Kind::Offset, args)
    }

    fn object_selection_complete(
        &self,
        document: &Document,
        _args: &[&str],
    ) -> Result<bool, CommandError> {
        Ok(document.selected_object_count() == 1
            && document
                .selected_objects()
                .all(|o| o.geometry().curve_ref().is_some()))
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
        let sources = selected_curves(document)?;
        let (target, parsed_distance) = match options.mode {
            OffsetMode::Distance(distance) if options.both_sides => {
                (Target::BothSides, Some(distance))
            }
            OffsetMode::Distance(distance) => (
                Target::Side(options.side.expect("validated side point")),
                Some(distance),
            ),
            OffsetMode::ThroughPoint(point) => (Target::Through(point), None),
        };
        let settings = Settings {
            distance: parsed_distance.unwrap_or_else(|| self.0.get().distance),
            corner: options.corner.into(),
            both_sides: options.both_sides,
            input_layer: options.input_layer,
            ..Settings::default()
        };
        let draft = Draft {
            kind: Kind::Offset,
            settings,
            normal: context.construction_plane.z_axis(),
            tolerance: document.tolerance(),
            target,
        };
        let plan = draft.solve(&sources)?;
        let count = admit(document, plan.curves, options.input_layer)?;
        self.0.set(Settings {
            distance: plan.distance.unwrap_or(settings.distance),
            corner: options.corner.into(),
            input_layer: options.input_layer,
            ..self.0.get()
        });
        Ok(match options.mode {
            OffsetMode::Distance(distance) => {
                format!("Created {count} offset curve(s) at distance {distance}",)
            }
            OffsetMode::ThroughPoint(point) => format!(
                "Created {count} offset curve(s) through {}",
                format_point(point),
            ),
        })
    }
}

fn selected_curves(document: &Document) -> Result<Vec<Curve3>, CommandError> {
    document
        .selected_objects()
        .map(|object| {
            object
                .geometry()
                .curve_ref()
                .map(viboceros_geometry::CurveRef::to_owned)
                .ok_or(CommandError::UnsupportedOffsetGeometry)
        })
        .collect()
}

fn admit(
    document: &mut Document,
    curves: Vec<(usize, Curve3)>,
    input_layer: bool,
) -> Result<usize, CommandError> {
    let layers = document
        .selected_objects()
        .map(|object| {
            ObjectAttributes::on_layer(if input_layer {
                object.attributes().layer_id()
            } else {
                document.current_layer_id()
            })
        })
        .collect::<Vec<_>>();
    let mut ids = Vec::with_capacity(curves.len());
    for (index, curve) in curves {
        ids.push(
            document.add_geometry_with_attributes(Geometry::from(curve), layers[index].clone())?,
        );
    }
    let count = ids.len();
    document.select_objects_direct(ids, SelectionMode::Replace)?;
    Ok(count)
}

fn map_offset_error(error: GeometryError) -> CommandError {
    if error == GeometryError::UnsupportedCurveOffset {
        CommandError::UnsupportedOffsetGeometry
    } else {
        error.into()
    }
}

#[derive(Clone, Copy)]
struct Options {
    mode: OffsetMode,
    side: Option<Point3>,
    both_sides: bool,
    input_layer: bool,
    corner: CurveOffsetCornerStyle,
}

#[derive(Clone, Copy)]
enum OffsetMode {
    Distance(Real),
    ThroughPoint(Point3),
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let first = *arguments.first().ok_or(CommandError::Usage(USAGE))?;
    let (mode, first_consumed) = if first.eq_ignore_ascii_case("ThroughPoint") {
        let (point, consumed) = parse_point(&arguments[1..])?;
        (OffsetMode::ThroughPoint(point), 1 + consumed)
    } else if let Some((name, value)) = first.split_once('=') {
        if !name.eq_ignore_ascii_case("ThroughPoint") {
            return Err(CommandError::Usage(USAGE));
        }
        let (point, _) = parse_point(&[value])?;
        (OffsetMode::ThroughPoint(point), 1)
    } else {
        let distance = parse_finite_real(first)?;
        if distance <= 0.0 {
            return Err(GeometryError::InvalidCurveOffsetDistance.into());
        }
        (OffsetMode::Distance(distance), 1)
    };
    let mut options = Options {
        mode,
        side: None,
        both_sides: false,
        input_layer: false,
        corner: CurveOffsetCornerStyle::Sharp,
    };
    let mut both_seen = false;
    let mut layer_seen = false;
    let mut corner_seen = false;
    let mut index = first_consumed;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else if argument.eq_ignore_ascii_case("BothSides")
            || argument.eq_ignore_ascii_case("OutputLayer")
            || argument.eq_ignore_ascii_case("Corner")
        {
            (
                argument,
                *arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        } else {
            if options.side.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (point, consumed) = parse_point(&arguments[index..])?;
            options.side = Some(point);
            index += consumed;
            continue;
        };
        if name.eq_ignore_ascii_case("BothSides") && !both_seen {
            options.both_sides = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            both_seen = true;
        } else if name.eq_ignore_ascii_case("OutputLayer") && !layer_seen {
            options.input_layer = getter::layer(value).ok_or(CommandError::Usage(USAGE))?;
            layer_seen = true;
        } else if name.eq_ignore_ascii_case("Corner") && !corner_seen {
            options.corner = getter::corner(value).ok_or(CommandError::Usage(USAGE))?;
            corner_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    match options.mode {
        OffsetMode::Distance(_) if options.both_sides == options.side.is_some() => {
            return Err(CommandError::Usage(USAGE));
        }
        OffsetMode::ThroughPoint(_) if options.both_sides || options.side.is_some() => {
            return Err(CommandError::Usage(USAGE));
        }
        _ => {}
    }
    Ok(options)
}
