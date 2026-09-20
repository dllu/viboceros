//! Object alignment: option state, tight bounds, and atomic rigid-unit edits.
use crate::remembered::Remembered;
use crate::{
    ChoiceSelectionOption, Command, CommandContext, CommandError, ObjectSelectionFilter,
    ObjectSelectionPrompt, ObjectSelectionWorkflow, option_name_eq, parse_point,
};
use viboceros_document::Document;
use viboceros_geometry::{AffineTransform3, BoundingBox3, Frame3, Point3, Vector3};

mod options;
mod projection;
pub use options::{AlignmentMode, AlignmentOptions};
#[cfg(test)]
mod tests;

const USAGE: &str = "Align mode [AlignTo=CPlane|World] [point|Auto]; ToLine start end; ToPlane [3Point] start end [third]";

#[derive(Default)]
pub(super) struct AlignCommand {
    world: Remembered<bool>,
}

impl AlignCommand {
    fn options(&self, arguments: &[&str]) -> Result<AlignmentOptions, CommandError> {
        AlignmentOptions {
            world: self.world.get(),
            ..Default::default()
        }
        .parse(arguments)
    }
}

impl Command for AlignCommand {
    fn name(&self) -> &'static str {
        "Align"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.options(arguments)?;
        if options.ready() && options.mode != Some(AlignmentMode::ToFitPlane)
            || options.references.iter().any(Option::is_some)
        {
            return Ok(None);
        }
        let mut choices = vec![ChoiceSelectionOption {
            name: "AlignTo",
            value: if options.world { "World" } else { "CPlane" },
            choices: &["CPlane", "World"],
            toggle: None,
        }];
        if let Some(mode) = options.mode {
            choices.push(ChoiceSelectionOption {
                name: "Mode",
                value: mode.label(),
                choices: AlignmentMode::LABELS,
                toggle: None,
            });
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "Align",
            filter: ObjectSelectionFilter::Any,
            options: if options.mode == Some(AlignmentMode::ToPlane) {
                vec![crate::BooleanSelectionOption {
                    name: "3Point",
                    value: options.three_point,
                    aliases: &[],
                }]
            } else {
                vec![]
            },
            menus: vec![],
            choices,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.world.set(self.options(arguments)?.world);
        Ok(())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let message = self.run_in_context(document, arguments, context)?;
        document.clear_selection();
        Ok(message)
    }
    fn cleanup_failed_postselection(&self, document: &mut Document, error: &CommandError) {
        if matches!(
            error,
            CommandError::InsufficientPlaneAlignmentObjects { .. }
        ) {
            document.clear_selection();
        }
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = self.options(arguments)?;
        let mode = options.mode.ok_or(CommandError::Usage(USAGE))?;
        if options.projects() {
            if document.selected_object_ids().next().is_none() {
                return Err(CommandError::NoObjectsSelected);
            }
            self.world.set(options.world);
            return projection::run(document, options, context);
        }
        let units = crate::layout_units::selected_units(document);
        let first = units.first().ok_or(CommandError::NoObjectsSelected)?;
        self.world.set(options.world);
        let orientation = if options.world {
            CommandContext::default().construction_plane
        } else {
            context.construction_plane
        };
        // The display CPlane origin must not discard small extents in a distant model.
        let frame = orientation.with_origin(first[0].geometry().bounds().center()?);
        let bounds = units
            .iter()
            .map(|unit| {
                crate::object_bounds::local_bounds(
                    unit.iter().map(|object| object.geometry()),
                    frame,
                    document.tolerance(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let combined = bounds
            .iter()
            .copied()
            .skip(1)
            .try_fold(bounds[0], BoundingBox3::union)?;
        let target = options
            .target
            .map(|point| frame.coordinates_of(point))
            .transpose()?;
        let mut replacements = Vec::new();
        for (unit, bounds) in units.iter().zip(&bounds) {
            let offset = offsets(mode, *bounds, combined, target);
            if offset == [0.; 2] {
                continue;
            }
            let translation = translation(frame, offset)?;
            let transform = AffineTransform3::from_translation(translation);
            for object in unit {
                replacements.push((
                    object.id(),
                    object
                        .geometry()
                        .transformed(transform, document.tolerance())?,
                ));
            }
        }
        let count = units.len();
        let changed = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Aligned {count} object/group unit(s) {}; moved {changed} object(s)",
            mode.label()
        ))
    }
}

fn offsets(
    mode: AlignmentMode,
    bounds: BoundingBox3,
    all: BoundingBox3,
    target: Option<[f64; 3]>,
) -> [f64; 2] {
    let a = bounds.min().to_array();
    let b = bounds.max().to_array();
    let low = all.min().to_array();
    let high = all.max().to_array();
    let coordinate = |axis: usize, edge: i8| {
        let position = |lo: f64, hi: f64| match edge {
            -1 => lo,
            1 => hi,
            _ => lo.midpoint(hi),
        };
        target.map_or_else(|| position(low[axis], high[axis]), |p| p[axis])
            - position(a[axis], b[axis])
    };
    use AlignmentMode::*;
    match mode {
        Left => [coordinate(0, -1), 0.],
        Right => [coordinate(0, 1), 0.],
        Bottom => [0., coordinate(1, -1)],
        Top => [0., coordinate(1, 1)],
        VertCenter => [coordinate(0, 0), 0.],
        HorizCenter => [0., coordinate(1, 0)],
        Concentric => [coordinate(0, 0), coordinate(1, 0)],
        ToLine | ToPlane | ToFitPlane => {
            unreachable!("projection modes use individual object anchors")
        }
    }
}

fn translation(
    frame: Frame3,
    offset: [f64; 2],
) -> Result<Vector3, viboceros_geometry::GeometryError> {
    let a = frame.x_axis().as_vector().scaled(offset[0])?.to_array();
    let b = frame.y_axis().as_vector().scaled(offset[1])?.to_array();
    Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))
}
