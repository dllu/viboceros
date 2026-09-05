//! Construction-plane-dependent affine commands; document mutation is shared.
use super::*;

pub(super) struct ScaleTwoDimensionalCommand;

impl Command for ScaleTwoDimensionalCommand {
    fn name(&self) -> &'static str {
        "Scale2D"
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
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SCALE_2D_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let factor = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_nonzero_scale(remaining[0])?
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                SCALE_2D_USAGE,
            )?;
            scale_factor_from_reference(center, reference, target, document.tolerance())?
        };
        let frame = context.construction_plane.with_origin(center);
        let transform = AffineTransform3::try_frame_mapping(frame, frame, [factor, factor, 1.0])?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) in two dimensions by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

pub(super) struct RotateCommand;

impl Command for RotateCommand {
    fn name(&self) -> &'static str {
        "Rotate"
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
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, ROTATE_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                ROTATE_USAGE,
            )?;
            plane_angle(
                context.construction_plane,
                center,
                reference,
                target,
                document.tolerance(),
            )?
        };
        let axis = context.construction_plane.z_axis();
        let transform = AffineTransform3::try_rotation(center, axis, angle_radians)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Rotated {transformed} object(s) by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees(),
        ))
    }
}

pub(super) struct MirrorCommand;

impl Command for MirrorCommand {
    fn name(&self) -> &'static str {
        "Mirror"
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
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, MIRROR_USAGE)?;
        let (axis_start, consumed) = parse_point(&positional)?;
        let (axis_end, end_consumed) = parse_point(&positional[consumed..])?;
        require_consumed(&positional, consumed + end_consumed, MIRROR_USAGE)?;
        let normal = context
            .construction_plane
            .z_axis()
            .as_vector()
            .cross(plane_vector(
                context.construction_plane,
                axis_start,
                axis_end,
            )?)?
            .normalized(document.tolerance())?;
        let transform = AffineTransform3::try_reflection(axis_start, normal)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Mirrored {transformed} object(s), creating {copied} copy object(s)"
        ))
    }
}

pub(super) struct ShearCommand;

impl Command for ShearCommand {
    fn name(&self) -> &'static str {
        "Shear"
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
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SHEAR_USAGE)?;
        let (origin, origin_consumed) = parse_point(&positional)?;
        let (reference, reference_consumed) = parse_point(&positional[origin_consumed..])?;
        let consumed = origin_consumed + reference_consumed;
        let remaining = &positional[consumed..];
        let reference_vector = origin.vector_to(reference)?;
        let reference_unit = reference_vector
            .normalized(document.tolerance())?
            .as_vector();
        let plane = context.construction_plane;
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (target, target_consumed) = parse_point(remaining)?;
            require_consumed(remaining, target_consumed, SHEAR_USAGE)?;
            // Shear's picked angle is spatial; its sign comes from the
            // construction-plane normal. atan2 avoids acos's near-parallel
            // loss of precision, while retaining the measured 3D angle.
            let target_unit = origin
                .vector_to(target)?
                .normalized(document.tolerance())?
                .as_vector();
            let cross = reference_unit.cross(target_unit)?;
            let angle = cross
                .length()?
                .atan2(reference_unit.dot(target_unit)?.clamp(-1.0, 1.0));
            // Parallel projected references use the positive branch. Ignore
            // only unit-vector roundoff here: a tiny spurious negative sign
            // must not reverse a large spatial shear angle.
            if projected_turn_sign(plane, reference_unit, target_unit)? < 0.0 {
                -angle
            } else {
                angle
            }
        };
        let reference_direction = plane_vector(context.construction_plane, origin, reference)?
            .normalized(document.tolerance())?;
        let shear_direction = context
            .construction_plane
            .z_axis()
            .as_vector()
            .cross(reference_direction.as_vector())?
            .normalized_nonzero()?;
        // Rhino's reference direction is unitized before its projection.
        // Its horizontal length is therefore the required obliquity scale.
        let in_plane = reference_unit
            .dot(plane.x_axis().as_vector())?
            .hypot(reference_unit.dot(plane.y_axis().as_vector())?);
        let factor = angle_radians.tan() / in_plane;
        let transform = AffineTransform3::try_shear(
            origin,
            reference_direction,
            shear_direction,
            factor,
            document.tolerance(),
        )?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Sheared {transformed} object(s) by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees()
        ))
    }
}

const PROJECT_TO_CPLANE_USAGE: &str = "ProjectToCPlane [DeleteInput=Yes|No]";

pub(super) struct ProjectToConstructionPlaneCommand;

impl Command for ProjectToConstructionPlaneCommand {
    fn name(&self) -> &'static str {
        "ProjectToCPlane"
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
        let selected = selected_ids(document)?;
        let delete_input =
            parse_delete_input(arguments, PROJECT_TO_CPLANE_USAGE, &["DeleteInput"])?;
        let plane = context.construction_plane;
        let origin = plane.origin();
        let normal = plane.z_axis();
        let transform = AffineTransform3::try_planar_projection(Plane::new(origin, normal))?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, !delete_input)?;
        Ok(format!(
            "Projected {transformed} object(s) to the construction plane, creating {copied} copy object(s)"
        ))
    }
}

fn plane_vector(plane: Frame3, origin: Point3, target: Point3) -> Result<Vector3, GeometryError> {
    let [x, y, _] = plane.with_origin(origin).coordinates_of(target)?;
    let zero = Point3::try_new(0.0, 0.0, 0.0)?;
    zero.vector_to(plane.with_origin(zero).point_at([x, y, 0.0])?)
}

fn plane_angle(
    plane: Frame3,
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let from = plane_vector(plane, center, reference)?
        .normalized(tolerance)?
        .as_vector();
    let to = plane_vector(plane, center, target)?
        .normalized(tolerance)?
        .as_vector();
    let cosine = from.dot(to)?.clamp(-1.0, 1.0);
    let sine = plane
        .z_axis()
        .as_vector()
        .dot(from.cross(to)?)?
        .clamp(-1.0, 1.0);
    Ok(sine.atan2(cosine))
}

fn projected_turn_sign(plane: Frame3, from: Vector3, to: Vector3) -> Result<Real, GeometryError> {
    let projected_unit = |vector: Vector3| -> Result<Option<[Real; 2]>, GeometryError> {
        let axes = [plane.x_axis().as_vector(), plane.y_axis().as_vector()];
        let coordinates = [vector.dot(axes[0])?, vector.dot(axes[1])?];
        // Bound cancellation per dot product, not by the spatial vector's
        // length: a tiny XY direction can still be perfectly resolved when
        // a huge normal component multiplies exact axis zeros.
        let uncertainty = axes.map(|axis| {
            8.0 * Real::EPSILON
                * vector
                    .to_array()
                    .into_iter()
                    .zip(axis.to_array())
                    .map(|(a, b)| (a * b).abs())
                    .sum::<Real>()
        });
        if coordinates
            .iter()
            .zip(uncertainty)
            .all(|(v, e)| v.abs() <= e)
        {
            return Ok(None);
        }
        let length = coordinates[0].hypot(coordinates[1]);
        Ok(Some(coordinates.map(|v| v / length)))
    };
    let (Some(from), Some(to)) = (projected_unit(from)?, projected_unit(to)?) else {
        return Ok(1.0);
    };
    let sine = from[0].mul_add(to[1], -from[1] * to[0]);
    Ok(if sine < -8.0 * Real::EPSILON {
        -1.0
    } else {
        1.0
    })
}
