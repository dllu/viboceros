//! Rectangular, linear, and polar copies; shared document transactions retain
//! original selection, attributes, and independent copied group topology.
use super::*;
#[cfg(test)]
mod tests;

pub(super) const ARRAY_USAGE: &str =
    "Array x-count y-count z-count x-distance y-distance z-distance [Mode=UnitCell|Fill]";

pub(super) struct ArrayCommand;

impl Command for ArrayCommand {
    fn name(&self) -> &'static str {
        "Array"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ArrayRectangular"]
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
        if arguments.len() < 6 {
            return Err(CommandError::Usage(ARRAY_USAGE));
        }
        let counts = [
            parse_array_dimension_count(arguments[0])?,
            parse_array_dimension_count(arguments[1])?,
            parse_array_dimension_count(arguments[2])?,
        ];
        let distances = [
            parse_finite_real(arguments[3])?,
            parse_finite_real(arguments[4])?,
            parse_finite_real(arguments[5])?,
        ];
        let mode = parse_rectangular_array_mode(&arguments[6..])?;
        let selected = selected_ids(document)?;
        let source_count = selected.len();
        let cell_count = counts
            .into_iter()
            .try_fold(1_usize, |product, count| product.checked_mul(count))
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let copy_instance_count = cell_count - 1;
        selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let spacing = match mode {
            RectangularArrayMode::UnitCell => distances,
            RectangularArrayMode::Fill => rectangular_fill_spacing(
                selected_plane_bounds(document, &selected, context.construction_plane)?,
                counts,
                distances,
            )?,
        };
        let mut transforms = Vec::new();
        transforms
            .try_reserve_exact(copy_instance_count)
            .map_err(|_| CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        for z_index in 0..counts[2] {
            for y_index in 0..counts[1] {
                for x_index in 0..counts[0] {
                    if x_index == 0 && y_index == 0 && z_index == 0 {
                        continue;
                    }
                    let offset = context.construction_plane.vector_at([
                        spacing[0] * x_index as Real,
                        spacing[1] * y_index as Real,
                        spacing[2] * z_index as Real,
                    ])?;
                    // Suppress cells coincident with the originals, but keep
                    // repeated nonzero placements when an axis spacing is zero.
                    if offset.to_array() != [0.; 3] {
                        transforms.push(AffineTransform3::from_translation(offset));
                    }
                }
            }
        }
        let copy_count = selected.len() * transforms.len();
        let copies = copy_array_objects(document, &selected, &transforms)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {}×{}×{} cells using {} distances {:.6},{:.6},{:.6}, creating {copy_count} copy object(s)",
            source_count,
            counts[0],
            counts[1],
            counts[2],
            mode.name(),
            distances[0],
            distances[1],
            distances[2],
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RectangularArrayMode {
    UnitCell,
    Fill,
}

impl RectangularArrayMode {
    const fn name(self) -> &'static str {
        match self {
            Self::UnitCell => "UnitCell",
            Self::Fill => "Fill",
        }
    }
}

fn parse_rectangular_array_mode(arguments: &[&str]) -> Result<RectangularArrayMode, CommandError> {
    if arguments.is_empty() {
        return Ok(RectangularArrayMode::UnitCell);
    }
    let (name, value) = match arguments {
        [option] => option
            .split_once('=')
            .ok_or(CommandError::Usage(ARRAY_USAGE))?,
        [name, value] => (*name, *value),
        _ => return Err(CommandError::Usage(ARRAY_USAGE)),
    };
    if !name.trim_start_matches('_').eq_ignore_ascii_case("Mode") {
        return Err(CommandError::Usage(ARRAY_USAGE));
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("UnitCell") {
        Ok(RectangularArrayMode::UnitCell)
    } else if value.eq_ignore_ascii_case("Fill") {
        Ok(RectangularArrayMode::Fill)
    } else {
        Err(CommandError::Usage(ARRAY_USAGE))
    }
}

fn parse_array_dimension_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidArrayDimensionCount(value.to_owned()))
}

fn rectangular_fill_spacing(
    bounds: BoundingBox3,
    counts: [usize; 3],
    lengths: [Real; 3],
) -> Result<[Real; 3], CommandError> {
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let mut spacing = [0.0; 3];
    for axis in 0..3 {
        if counts[axis] == 1 {
            continue;
        }
        let object_extent = max[axis] - min[axis];
        if !object_extent.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "rectangular array bounds",
            }
            .into());
        }
        if lengths[axis] == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "array fill length",
            }
            .into());
        }
        // Rhino measures a signed endpoint from the lower bound, then takes
        // the absolute gap to the upper bound while retaining the input sign.
        // Small positive lengths are valid; negative lengths add the extent.
        spacing[axis] = lengths[axis].signum() * (lengths[axis] - object_extent).abs()
            / (counts[axis] - 1) as Real;
    }
    Ok(spacing)
}

fn selected_plane_bounds(
    document: &Document,
    selected: &[ObjectId],
    plane: Frame3,
) -> Result<BoundingBox3, CommandError> {
    let world = CommandContext::default().construction_plane;
    // Fill needs extents, not a location relative to the CPlane origin. Keeping
    // the origin out of this map also prevents a distant plane from swallowing
    // small source dimensions during subtraction.
    let plane = plane.with_origin(world.origin());
    let to_local = AffineTransform3::try_frame_mapping(plane, world, [1.; 3])?;
    let bounds = |id| -> Result<BoundingBox3, CommandError> {
        let source = document
            .object(id)
            .expect("selected source exists")
            .geometry();
        let geometry = if plane == world {
            std::borrow::Cow::Borrowed(source)
        } else {
            std::borrow::Cow::Owned(source.transformed(to_local, document.tolerance())?)
        };
        Ok(if let Some(curve) = geometry.curve_ref() {
            curve.tight_bounds(document.tolerance())?
        } else if let Geometry::NurbsSurface(surface) = geometry.as_ref() {
            surface.tight_bounds(document.tolerance())?
        } else {
            geometry.bounds()
        })
    };
    let (first, rest) = selected
        .split_first()
        .ok_or(CommandError::NoObjectsSelected)?;
    rest.iter()
        .try_fold(bounds(*first)?, |acc, id| Ok(acc.union(bounds(*id)?)?))
}

pub(super) struct ArrayLinearCommand;

impl Command for ArrayLinearCommand {
    fn name(&self) -> &'static str {
        "ArrayLinear"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let item_count_text = arguments.first().ok_or(CommandError::Usage(
            "ArrayLinear item-count first-reference second-reference",
        ))?;
        let item_count = parse_array_item_count(item_count_text)?;
        let selected = selected_ids(document)?;
        let copy_instance_count = item_count - 1;
        let copy_count = selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let (first_reference, first_consumed) = parse_point(&arguments[1..])?;
        let (second_reference, second_consumed) = parse_point(&arguments[1 + first_consumed..])?;
        require_consumed(
            arguments,
            1 + first_consumed + second_consumed,
            "ArrayLinear item-count first-reference second-reference",
        )?;
        let spacing = first_reference.vector_to(second_reference)?;
        let transforms = (1..item_count)
            .map(|index| {
                spacing
                    .scaled(index as Real)
                    .map(AffineTransform3::from_translation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let copies = copy_array_objects(document, &selected, &transforms)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {item_count} total item(s), creating {copy_count} copy object(s) at spacing {}",
            copy_count / copy_instance_count,
            format_vector(spacing)
        ))
    }
}

pub(super) const ARRAY_POLAR_USAGE: &str =
    "ArrayPolar item-count center angle-degrees [Rotate=Yes|No] [ZOffset=distance]";

pub(super) struct ArrayPolarCommand;

impl Command for ArrayPolarCommand {
    fn name(&self) -> &'static str {
        "ArrayPolar"
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
        let item_count_text = arguments
            .first()
            .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
        let item_count = parse_array_item_count(item_count_text)?;
        let selected = selected_ids(document)?;
        let copy_instance_count = item_count - 1;
        let copy_count = selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let (center, center_consumed) = parse_point(&arguments[1..])?;
        let angle_index = 1 + center_consumed;
        let angle_text = arguments
            .get(angle_index)
            .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
        let fill_angle_degrees = parse_finite_real(angle_text)?;
        if fill_angle_degrees == 0.0 {
            return Err(CommandError::InvalidPolarArrayAngle(
                (*angle_text).to_owned(),
            ));
        }
        let options = parse_polar_array_options(&arguments[angle_index + 1..])?;
        let axis = context.construction_plane.z_axis();
        let divisor = if fill_angle_degrees.abs() == 360.0 {
            item_count
        } else {
            copy_instance_count
        };
        let step_radians = (fill_angle_degrees / divisor as Real).to_radians();
        let anchor = if options.rotate {
            None
        } else {
            Some(
                selected_plane_bounds(
                    document,
                    &selected,
                    CommandContext::default().construction_plane,
                )?
                .center()?,
            )
        };
        let transforms = (1..item_count)
            .map(|index| {
                let rotation =
                    AffineTransform3::try_rotation(center, axis, step_radians * index as Real)?;
                let z_offset = axis.as_vector().scaled(options.z_offset * index as Real)?;
                if let Some(anchor) = anchor {
                    let destination = rotation.transform_point(anchor)?.translated(z_offset)?;
                    Ok(AffineTransform3::from_translation(
                        anchor.vector_to(destination)?,
                    ))
                } else {
                    post_translate(rotation, z_offset)
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let copies = copy_array_objects(document, &selected, &transforms)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {item_count} total item(s) over {fill_angle_degrees:.6} degrees, creating {copy_count} copy object(s)",
            copy_count / copy_instance_count
        ))
    }
}

#[derive(Clone, Copy)]
struct PolarArrayOptions {
    rotate: bool,
    z_offset: Real,
}

fn parse_polar_array_options(arguments: &[&str]) -> Result<PolarArrayOptions, CommandError> {
    let mut options = PolarArrayOptions {
        rotate: true,
        z_offset: 0.0,
    };
    let mut rotate_seen = false;
    let mut z_offset_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        if name.eq_ignore_ascii_case("Rotate") && !rotate_seen {
            options.rotate = parse_yes_no(value).ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
            rotate_seen = true;
        } else if name.eq_ignore_ascii_case("ZOffset") && !z_offset_seen {
            options.z_offset = parse_finite_real(value)?;
            z_offset_seen = true;
        } else {
            return Err(CommandError::Usage(ARRAY_POLAR_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn parse_array_item_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 2)
        .ok_or_else(|| CommandError::InvalidArrayItemCount(value.to_owned()))
}

fn post_translate(
    transform: AffineTransform3,
    offset: Vector3,
) -> Result<AffineTransform3, GeometryError> {
    let translation = transform.translation();
    AffineTransform3::try_new(
        transform.linear_rows(),
        Vector3::try_new(
            translation.x() + offset.x(),
            translation.y() + offset.y(),
            translation.z() + offset.z(),
        )?,
    )
}

fn copy_array_objects(
    document: &mut Document,
    selected: &[ObjectId],
    transforms: &[AffineTransform3],
) -> Result<Vec<ObjectId>, DocumentError> {
    use viboceros_document::CopyGroupPolicy;
    // Rhino's single-object array path clears groups on the copies, even
    // though the original remains grouped. Multi-object arrays copy the full
    // selected group graph, including one-member groups.
    let policy = if selected.len() == 1 {
        CopyGroupPolicy::Omit
    } else {
        CopyGroupPolicy::Preserve
    };
    let copies = document.copy_objects_with_transforms_and_groups(
        selected.iter().copied(),
        transforms,
        policy,
    )?;
    document.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
    Ok(copies)
}
