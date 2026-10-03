//! Move/Copy placement and replay policy, independent of point-prompt lifetime.
use super::*;
use viboceros_geometry::{UnitVector3, Vector3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyOptions {
    pub from_last_point: bool,
    pub use_last_distance: bool,
    pub use_last_direction: bool,
}

impl CopyOptions {
    /// Only repetition exposes these options. Invalid input is not accepted.
    pub fn update(&mut self, input: &str) -> Result<(), CommandError> {
        let (name, value) = input
            .split_once('=')
            .ok_or(CommandError::Usage(COPY_USAGE))?;
        let value = parse_yes_no(value).ok_or(CommandError::Usage(COPY_USAGE))?;
        let target = if option_name_eq(name, "FromLastPoint") {
            &mut self.from_last_point
        } else if option_name_eq(name, "UseLastDistance") {
            &mut self.use_last_distance
        } else if option_name_eq(name, "UseLastDirection") {
            &mut self.use_last_direction
        } else {
            return Err(CommandError::Usage(COPY_USAGE));
        };
        *target = value;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DestinationConstraint {
    pub anchor: Point3,
    pub direction: Option<UnitVector3>,
    pub distance: Option<Real>,
}

impl DestinationConstraint {
    /// Closest point on the constraint line to a free pick's viewing line.
    /// `origin_offset` is the vector from the anchor to the viewing-line origin,
    /// allowing callers to retain precision in a camera-local coordinate frame.
    /// Snapped and typed 3D points instead use `resolve`.
    pub fn project_view_line(
        self,
        origin_offset: Vector3,
        view_direction: Vector3,
    ) -> Option<Point3> {
        let axis = self.direction?.as_vector();
        let view = view_direction.normalized_nonzero().ok()?.as_vector();
        let cross = axis.cross(view).ok()?;
        let sine = cross.length().ok()?;
        if sine <= 1e-12 {
            return None;
        }
        let normal = cross.normalized_nonzero().ok()?.as_vector();
        let amount = origin_offset.cross(view).ok()?.dot(normal).ok()? / sine;
        let anchor = self.anchor.to_array();
        let axis = axis.to_array();
        Point3::try_from(std::array::from_fn(|i| axis[i].mul_add(amount, anchor[i]))).ok()
    }

    pub fn resolve(self, candidate: Point3) -> Result<Point3, GeometryError> {
        let candidate = if let Some(direction) = self.direction {
            let vector = direction.as_vector();
            let amount = self.anchor.vector_to(candidate)?.dot(vector)?;
            self.anchor.translated(vector.scaled(amount)?)?
        } else {
            candidate
        };
        if let Some(distance) = self.distance {
            if distance == 0.0 || candidate == self.anchor {
                return Ok(self.anchor);
            }
            let direction = self.anchor.direction_to(candidate)?.as_vector().to_array();
            let anchor = self.anchor.to_array();
            Point3::try_from(std::array::from_fn(|axis| {
                direction[axis].mul_add(distance, anchor[axis])
            }))
        } else {
            Ok(candidate)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CopyPlacement {
    base: Point3,
    previous: Option<(Point3, Vector3)>,
    direction: Option<UnitVector3>,
    pub options: CopyOptions,
}

impl CopyPlacement {
    pub fn new(base: Point3) -> Self {
        Self {
            base,
            previous: None,
            direction: None,
            options: CopyOptions::default(),
        }
    }

    pub fn base(self) -> Point3 {
        self.base
    }

    pub fn anchor(self) -> Point3 {
        if self.options.from_last_point {
            self.previous.map_or(self.base, |(point, _)| point)
        } else {
            self.base
        }
    }

    pub fn constraint(self, vertical: Option<UnitVector3>) -> DestinationConstraint {
        DestinationConstraint {
            anchor: self.anchor(),
            direction: vertical.or_else(|| {
                self.options
                    .use_last_direction
                    .then_some(self.direction)
                    .flatten()
            }),
            distance: self
                .options
                .use_last_distance
                .then_some(self.previous)
                .flatten()
                .and_then(|(_, step)| step.length().ok())
                .filter(|distance| *distance > 0.0),
        }
    }

    /// Called only after committing geometry. Rejected points keep the previous
    /// accepted destination, distance, direction, and original source geometry.
    pub fn accept(&mut self, destination: Point3) -> Result<(), GeometryError> {
        let step = self.anchor().vector_to(destination)?;
        if !self.options.use_last_direction {
            self.direction = step.normalized_nonzero().ok();
        }
        self.previous = Some((destination, step));
        Ok(())
    }

    /// A Vertical first placement seeds a positively oriented CPlane Z line.
    pub fn use_direction(&mut self, direction: UnitVector3) {
        self.direction = Some(direction);
        self.options.use_last_direction = true;
    }
}

pub const MOVE_USAGE: &str = "Move from to [Vertical]";
pub const COPY_USAGE: &str = "Copy from target [target ...] [Vertical] | Copy InPlace";

pub(super) struct MoveCommand;
impl Command for MoveCommand {
    fn name(&self) -> &'static str {
        "Move"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["M"]
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
        let (mut arguments, sources) = transform_arguments(document, arguments, MOVE_USAGE)?;
        let vertical = strip_vertical(&mut arguments, MOVE_USAGE)?;
        let (base, consumed) = parse_point(&arguments)?;
        let (target, count) =
            if vertical && arguments.len() == consumed + 1 && !arguments[consumed].contains(',') {
                (
                    base.translated(
                        context
                            .construction_plane
                            .z_axis()
                            .as_vector()
                            .scaled(parse_finite_real(arguments[consumed])?)?,
                    )?,
                    1,
                )
            } else {
                parse_point(&arguments[consumed..])?
            };
        require_consumed(&arguments, consumed + count, MOVE_USAGE)?;
        let target = DestinationConstraint {
            anchor: base,
            direction: vertical.then_some(context.construction_plane.z_axis()),
            distance: None,
        }
        .resolve(target)?;
        let offset = base.vector_to(target)?;
        let (count, _) = apply_transform_with_renewal(
            document,
            &sources,
            AffineTransform3::from_translation(offset),
            false,
        )?;
        Ok(format!(
            "Moved {count} object(s) by {}",
            format_vector(offset)
        ))
    }
}

pub(super) struct CopyCommand;
impl Command for CopyCommand {
    fn name(&self) -> &'static str {
        "Copy"
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
        let (mut arguments, mut sources) = transform_arguments(document, arguments, COPY_USAGE)?;
        sources.release_selection_on_replay();
        if arguments.len() == 1 && option_name_eq(arguments[0], "InPlace") {
            let (_, copied) = apply_transform_with_renewal(
                document,
                &sources,
                AffineTransform3::identity(),
                true,
            )?;
            return Ok(format!("Copied {copied} object(s) in place"));
        }
        let mut vertical = strip_vertical(&mut arguments, COPY_USAGE)?;
        let (base, mut consumed) = parse_point(&arguments)?;
        let mut placement = CopyPlacement::new(base);
        let mut copied = 0;
        let mut placements = 0;
        while consumed < arguments.len() {
            let argument = arguments[consumed];
            if argument.contains('=') {
                if placements == 0 {
                    return Err(CommandError::Usage(COPY_USAGE));
                }
                placement.options.update(argument)?;
                consumed += 1;
                continue;
            }
            let constraint =
                placement.constraint(vertical.then_some(context.construction_plane.z_axis()));
            let (target, count) = if let Some(direction) = constraint.direction
                && !argument.contains(',')
                && argument.parse::<Real>().is_ok()
            {
                (
                    constraint
                        .anchor
                        .translated(direction.as_vector().scaled(parse_finite_real(argument)?)?)?,
                    1,
                )
            } else {
                let (target, count) = parse_point(&arguments[consumed..])?;
                (constraint.resolve(target)?, count)
            };
            let offset = base.vector_to(target)?;
            let (_, created) = apply_transform_with_renewal(
                document,
                &sources,
                AffineTransform3::from_translation(offset),
                true,
            )?;
            copied += created;
            placements += 1;
            placement.accept(target)?;
            if vertical {
                placement.use_direction(context.construction_plane.z_axis());
                vertical = false;
            }
            consumed += count;
        }
        if placements == 0 {
            return Err(CommandError::Usage(COPY_USAGE));
        }
        Ok(format!(
            "Copied {copied} object(s) in {placements} placement(s)"
        ))
    }
}

fn strip_vertical(arguments: &mut Vec<&str>, usage: &'static str) -> Result<bool, CommandError> {
    let mut vertical = None;
    let mut invalid = false;
    arguments.retain(|argument| {
        let value = if option_name_eq(argument, "Vertical") {
            Some(true)
        } else if let Some((name, value)) = argument.split_once('=')
            && option_name_eq(name, "Vertical")
        {
            match parse_yes_no(value) {
                Some(value) => Some(value),
                None => {
                    invalid = true;
                    None
                }
            }
        } else {
            None
        };
        if let Some(value) = value {
            if vertical.replace(value).is_some() {
                invalid = true;
            }
            false
        } else {
            true
        }
    });
    if invalid {
        Err(CommandError::Usage(usage))
    } else {
        Ok(vertical.unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_copy_batch_rolls_back_earlier_placements_and_group_allocation_on_error() {
        let registry = CommandRegistry::with_builtins();
        for input in [
            "Copy 0,0,0 1,0,0 UseLastDistance=Maybe",
            "Copy 0,0,0 1,0,0 1e308,0,0",
        ] {
            let mut document = Document::default();
            let sources = [1e308, 2.].map(|x| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                    .unwrap()
            });
            document.add_group(None, sources).unwrap();
            document
                .select_objects_direct(sources, SelectionMode::Replace)
                .unwrap();
            document.clear_history().unwrap();
            let objects = document.objects().cloned().collect::<Vec<_>>();
            let groups = document.groups().cloned().collect::<Vec<_>>();
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
            assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
            assert_eq!(document.selected_object_count(), 2);
            assert!(!document.can_undo());
            registry.execute(&mut document, "Copy InPlace").unwrap();
            assert_eq!(document.groups().len(), 2);
            assert_eq!(document.objects().len(), 4);
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.selected_object_count(), 0);
            assert!(!document.can_undo());
        }
    }
}
