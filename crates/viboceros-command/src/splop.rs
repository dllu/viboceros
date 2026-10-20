//! Sphere-sized placement through the shared plane-to-surface point morph.
use super::*;
mod reference;
pub use reference::ReferenceInput;
use viboceros_document::{ControlPointId, CopyGroupPolicy, ReplacementHistory};
use viboceros_geometry::{PointMorph, SurfacePointMorph};

pub const USAGE: &str = "Splop base radius-or-point target radius-or-point [Surface=id] [Face=index] [Angle=degrees] [Copy=Yes|No] [Rigid=Yes|No] [Flip=Yes|No] [Finish=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
    pub flip: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            copy: true,
            rigid: false,
            flip: false,
        }
    }
}
impl Options {
    pub fn accept(&mut self, input: &str) -> Result<bool, CommandError> {
        let Some((name, value)) = input.split_once('=') else {
            return Ok(false);
        };
        let field = if option_name_eq(name, "Copy") {
            &mut self.copy
        } else if option_name_eq(name, "Rigid") {
            &mut self.rigid
        } else if option_name_eq(name, "Flip") {
            &mut self.flip
        } else {
            return Ok(false);
        };
        *field = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        Ok(true)
    }
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Copy={} Rigid={} Flip={}",
            yes(self.copy),
            yes(self.rigid),
            yes(self.flip)
        )
    }
}

#[derive(Clone, Debug)]
pub struct ReferenceSphere {
    pub frame: Frame3,
    pub radius: Real,
}
impl ReferenceSphere {
    pub fn try_new(center: Point3, radius: Real, plane: Frame3) -> Result<Self, GeometryError> {
        if !radius.is_finite() || radius <= 0. {
            return Err(GeometryError::Degenerate {
                context: "Splop reference sphere radius",
            });
        }
        Ok(Self {
            frame: Frame3::try_from_unit_directions(
                center,
                plane.x_axis(),
                plane.y_axis(),
                Tolerance::NUMERICAL_VALIDATION,
            )?,
            radius,
        })
    }
    pub fn from_radius_point(
        center: Point3,
        point: Point3,
        plane: Frame3,
    ) -> Result<Self, GeometryError> {
        let radius = center.distance_to(point)?;
        let frame = Frame3::try_from_x_and_normal(
            center,
            center.vector_to(point)?,
            plane.z_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        Self::try_new(center, radius, frame)
    }
}

#[derive(Clone, Debug)]
pub struct Placement {
    pub reference: ReferenceSphere,
    pub target: orient_surface::Target,
    pub pose: orient_surface::Pose,
}
impl Placement {
    pub fn try_new(
        reference: ReferenceSphere,
        target: orient_surface::Target,
        point: Point3,
    ) -> Result<Self, CommandError> {
        let location = target.location(point, false)?;
        let pose = orient_surface::Pose {
            point: location.point,
            uv: location.uv,
            frame: location.surface.frame_at(
                location.uv[0],
                location.uv[1],
                Tolerance::NUMERICAL_VALIDATION,
            )?,
        };
        Ok(Self {
            reference,
            target,
            pose,
        })
    }
    pub fn morph(
        &self,
        radius: Real,
        angle: Real,
        options: Options,
        tolerance: Tolerance,
    ) -> Result<Morph, GeometryError> {
        if !radius.is_finite() || radius <= 0. || !angle.is_finite() {
            return Err(GeometryError::Degenerate {
                context: "Splop destination sphere",
            });
        }
        let mut morph = SurfacePointMorph::try_new_owned(
            self.reference.frame,
            self.target.surface.clone(),
            self.pose.uv[0],
            self.pose.uv[1],
            radius / self.reference.radius,
            angle.to_radians(),
            options.flip ^ self.target.reversed,
            tolerance,
        )?;
        if let Some(solver) = morph_interpolation_solver() {
            morph = morph.with_interpolation_solver(solver);
        }
        Ok(Morph {
            point_map: morph.with_normal_height_scale(if options.flip {
                1.
            } else {
                radius / self.reference.radius
            })?,
            scale: radius / self.reference.radius,
        })
    }
    pub fn sphere_at(
        &self,
        point: Point3,
        locked_radius: Option<Real>,
        options: Options,
    ) -> Result<(Real, Real), GeometryError> {
        let delta = self.pose.point.vector_to(point)?;
        let x = delta.dot(self.pose.frame.x_axis().as_vector())?;
        let y = delta.dot(self.pose.frame.y_axis().as_vector())?;
        if x == 0. && y == 0. {
            return Err(GeometryError::Degenerate {
                context: "Splop destination sphere direction",
            });
        }
        Ok((
            locked_radius.unwrap_or(delta.length()?),
            (if options.flip ^ self.target.reversed {
                -y
            } else {
                y
            })
            .atan2(x)
            .to_degrees(),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct Morph {
    point_map: SurfacePointMorph<'static>,
    scale: Real,
}
impl PointMorph for Morph {
    fn rebuild_grip_curve(
        &self,
        curve: viboceros_geometry::NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<viboceros_geometry::Curve3, GeometryError> {
        curve.into_curve_with_derivative_breaks(tolerance)
    }
    fn interpolation_solver(&self) -> Option<viboceros_geometry::MorphInterpolationSolver> {
        self.point_map.interpolation_solver()
    }
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        self.point_map.morph_point(p)
    }
}
impl Morph {
    pub fn rigid_transform(&self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        AffineTransform3::try_uniform_scale(center, self.scale)?
            .then(self.point_map.rigid_transform(center)?)
    }
}
impl crate::morph_edit::ObjectMorph for Morph {
    fn rigid_pose(&self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        self.rigid_transform(center)
    }
    fn preserving(self, _: bool) -> Self {
        self
    }
}

pub fn deformed_geometries(
    doc: &Document,
    ids: &[ObjectId],
    grips: &[ControlPointId],
    morph: &Morph,
    options: Options,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    crate::morph_edit::deformed(
        doc,
        ids,
        grips,
        morph,
        options.rigid,
        true,
        rigid_groups::GroupFrameOrigin::BoundingBoxCenter,
    )
}

pub(super) struct SplopCommand;
impl Command for SplopCommand {
    fn name(&self) -> &'static str {
        "Splop"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(true)
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (args, mut sources) = affine_transform_arguments(doc, args, USAGE)?;
        let mut options = Options::default();
        let mut positional = Vec::new();
        let mut surface = None;
        let mut face = None;
        let mut angle = 0.;
        let mut finish = true;
        let mut seen = BTreeSet::new();
        for word in args {
            if let Some((name, value)) = word.split_once('=') {
                let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
                if !seen.insert(name.clone()) {
                    return Err(CommandError::Usage(USAGE));
                }
                if options.accept(word)? {
                    continue;
                }
                match name.as_str() {
                    "surface" => {
                        surface = Some(
                            value
                                .parse::<ObjectId>()
                                .map_err(|_| CommandError::Usage(USAGE))?,
                        )
                    }
                    "face" => {
                        face = Some(
                            value
                                .parse::<usize>()
                                .map_err(|_| CommandError::Usage(USAGE))?,
                        )
                    }
                    "angle" => angle = parse_finite_real(value)?,
                    "finish" => finish = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            } else {
                positional.push(word);
            }
        }
        let (base, n) = parse_point(&positional)?;
        let (reference, consumed) = if positional.get(n).is_some_and(|v| v.contains(',')) {
            let (point, m) = parse_point(&positional[n..])?;
            (
                ReferenceSphere::from_radius_point(base, point, context.construction_plane)?,
                m,
            )
        } else {
            let radius = parse_finite_real(positional.get(n).ok_or(CommandError::Usage(USAGE))?)?;
            (
                ReferenceSphere::try_new(base, radius, context.construction_plane)?,
                1,
            )
        };
        let (target, m) = parse_point(&positional[n + consumed..])?;
        let end = n + consumed + m;
        let destination = positional.get(end).ok_or(CommandError::Usage(USAGE))?;
        let direction = if destination.contains(',') || positional.len() > end + 1 {
            let (point, n) = parse_point(&positional[end..])?;
            require_consumed(&positional, end + n, USAGE)?;
            if seen.contains("angle") {
                return Err(CommandError::Usage(USAGE));
            }
            Some(point)
        } else {
            None
        };
        let mut destination_radius = if direction.is_none() {
            require_consumed(&positional, end + 1, USAGE)?;
            parse_finite_real(destination)?
        } else {
            0.
        };
        let surface = surface.ok_or(CommandError::Usage(USAGE))?;
        sources.ids.retain(|id| *id != surface);
        if sources.grips.iter().any(|g| g.object == surface) {
            return Err(CommandError::Usage(
                "Splop destination cannot supply source grips",
            ));
        }
        if sources.ids.is_empty() && sources.grips.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let placement = Placement::try_new(
            reference,
            orient_surface::Target::capture(doc, surface, face)?,
            target,
        )?;
        if let Some(direction) = direction {
            (destination_radius, angle) = placement.sphere_at(direction, None, options)?;
        }
        let morph = placement.morph(destination_radius, angle, options, doc.tolerance())?;
        let staged = deformed_geometries(doc, &sources.ids, &sources.grips, &morph, options)?;
        let selection_before = doc.selected_object_ids().collect::<Vec<_>>();
        let ids = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let count = ids.len()
            + sources
                .grips
                .iter()
                .map(|g| g.object)
                .collect::<BTreeSet<_>>()
                .len();
        if options.copy {
            doc.retain_created_group_definitions_on_undo()?;
        }
        if sources.release_on_replay {
            doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
        }
        if !sources.grips.is_empty() {
            doc.morph_grips_with_object_geometries(
                staged,
                sources.grips.iter().copied(),
                &morph,
                options.copy,
                CopyGroupPolicy::Preserve,
            )?;
        } else if options.copy {
            doc.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
        } else {
            doc.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            doc.move_objects_to_end_in_order(ids)?;
        }
        if sources.postselected && (!options.copy || finish) {
            doc.clear_selection();
        } else if options.copy {
            // Ordinary copy insertion selects its outputs. Splop retains the
            // live source/reference picks until its outer getters finish.
            doc.select_command_results(selection_before)?;
        }
        Ok(format!(
            "Splopped {count} object(s){}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}

/// Finish an interactive copy batch while retaining its single Undo entry.
/// Preselected source picks survive Enter. Command-first picks are temporary.
/// Escape keeps accepted copies; the UI releases its command-first picks.
pub fn finish_copies(
    doc: &mut Document,
    group: &mut viboceros_document::HistoryGroup,
    sources: &[ObjectId],
    postselected: bool,
) -> Result<bool, DocumentError> {
    if !group.can_undo() {
        return Ok(false);
    }
    if !doc.history_group_is_current(group) {
        return Err(DocumentError::HistoryGroupStale);
    }
    if !postselected {
        return Ok(true);
    }
    doc.begin_group_transaction(group)?;
    if let Err(error) = doc.release_command_selection_on_history_replay(sources.iter().copied()) {
        doc.rollback_transaction()?;
        return Err(error);
    }
    doc.clear_selection();
    doc.commit_group_transaction(group)
}
