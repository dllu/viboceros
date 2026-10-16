//! Shared rigid/splop placement on a captured surface reference.
use super::*;
use std::sync::Arc;
use viboceros_document::GeometrySnapshot;
pub const USAGE: &str = "OrientOnSrf base reference target [Surface=id] [Face=index] [SurfaceName=name] [Scale=factor] [Rotation=degrees] [Rigid=Yes|No] [Copy=Yes|No] [Flip=Yes|No] [ConstrainNormal=Yes|No] [SourceNormal=x,y,z] [DestinationNormal=x,y,z] [OnSurface=Yes|No] [IgnoreTrims=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub scale: Real,
    pub rotation: Real,
    pub copy: bool,
    pub rigid: bool,
    pub flip: bool,
    pub constrain_normal: bool,
    pub ignore_trims: bool,
    pub on_surface: bool,
    pub source_normal: Option<Vector3>,
    pub destination_normal: Option<Vector3>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            scale: 1.,
            rotation: 0.,
            copy: true,
            rigid: true,
            flip: false,
            constrain_normal: false,
            ignore_trims: false,
            on_surface: false,
            source_normal: None,
            destination_normal: None,
        }
    }
}
impl Options {
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        let mut args = format!(
            "Scale={} Rotation={} Copy={} Rigid={} Flip={} ConstrainNormal={} IgnoreTrims={} OnSurface={}",
            self.scale,
            self.rotation,
            yes(self.copy),
            yes(self.rigid),
            yes(self.flip),
            yes(self.constrain_normal),
            yes(self.ignore_trims),
            yes(self.on_surface)
        );
        for (name, value) in [
            ("SourceNormal", self.source_normal),
            ("DestinationNormal", self.destination_normal),
        ] {
            if let Some(v) = value {
                let [x, y, z] = v.to_array();
                args += &format!(" {name}={x},{y},{z}");
            }
        }
        args
    }
}
#[derive(Default)]
pub struct Reference {
    pub id: Option<ObjectId>,
    pub face: Option<usize>,
    pub name: Option<String>,
}
pub fn options<'a>(
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options, Reference), CommandError> {
    let mut reference = Reference::default();
    let mut rest = Vec::new();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let (key, value) = args[i]
            .split_once('=')
            .map_or((args[i], None), |(k, v)| (k, Some(v)));
        let key = key.trim_start_matches(['_', '-']).to_ascii_lowercase();
        let key = match key.as_str() {
            "angle" => "rotation",
            "up" => "sourcenormal",
            _ => &key,
        };
        if [
            "scale",
            "rotation",
            "copy",
            "rigid",
            "flip",
            "constrainnormal",
            "ignoretrims",
            "onsurface",
            "sourcenormal",
            "destinationnormal",
            "surface",
            "face",
            "surfacename",
        ]
        .contains(&key)
        {
            if !seen.insert(key.to_owned()) {
                return Err(CommandError::Usage(USAGE));
            }
            let value = match value {
                Some(v) => v,
                None => {
                    i += 1;
                    *args.get(i).ok_or(CommandError::Usage(USAGE))?
                }
            };
            let toggle = || parse_yes_no(value).ok_or(CommandError::Usage(USAGE));
            match key {
                "scale" => {
                    options.scale = parse_finite_real(value)?;
                    if options.scale <= 0. {
                        return Err(CommandError::InvalidScaleFactor(value.into()));
                    }
                }
                "rotation" => {
                    options.rotation = parse_finite_real(value)?;
                    if !options.rotation.to_radians().is_finite() {
                        return Err(CommandError::InvalidNumber(value.into()));
                    }
                }
                "copy" => options.copy = toggle()?,
                "rigid" => options.rigid = toggle()?,
                "flip" => options.flip = toggle()?,
                "constrainnormal" => options.constrain_normal = toggle()?,
                "ignoretrims" => options.ignore_trims = toggle()?,
                "onsurface" => options.on_surface = toggle()?,
                "sourcenormal" | "destinationnormal" => {
                    let v = Vector3::try_from(parse_single_option_point(value, USAGE)?.to_array())?;
                    v.normalized_nonzero()?;
                    if key == "sourcenormal" {
                        options.source_normal = Some(v);
                    } else {
                        options.destination_normal = Some(v);
                    }
                }
                "surface" => {
                    reference.id = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "face" => {
                    reference.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "surfacename" if !value.is_empty() => reference.name = Some(value.into()),
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            rest.push(args[i]);
        }
        i += 1;
    }
    if reference.id.is_some() && reference.name.is_some() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((rest, options, reference))
}
#[derive(Clone, Debug)]
pub struct Target {
    pub id: ObjectId,
    pub face: usize,
    pub surface: Arc<NurbsSurface>,
    pub reversed: bool,
    parent: GeometrySnapshot,
    tolerance: Tolerance,
}
impl Target {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        face: Option<usize>,
    ) -> Result<Self, CommandError> {
        let object = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(DocumentError::ObjectNotFound(id))?;
        let (surface, face, reversed) = surface_reference::surface(object.geometry(), face)
            .ok_or(CommandError::SurfaceOrientTargetNotSurface)?;
        Ok(Self {
            id,
            face,
            surface: Arc::new(surface.clone()),
            reversed,
            parent: object.geometry_snapshot().clone(),
            tolerance: doc.tolerance(),
        })
    }
    pub fn source_normal(
        &self,
        point: Point3,
        ignore_trims: bool,
    ) -> Result<Vector3, CommandError> {
        let l = self.location(point, ignore_trims)?;
        Ok(l.surface
            .normal_at(l.uv[0], l.uv[1])?
            .as_vector()
            .scaled(if self.reversed { -1. } else { 1. })?)
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.id)
            && doc
                .object(self.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn location(
        &self,
        point: Point3,
        ignore_trims: bool,
    ) -> Result<surface_reference::SurfacePoint<'_>, CommandError> {
        surface_reference::closest(
            &self.parent,
            Some(self.face),
            point,
            ignore_trims,
            self.tolerance,
        )?
        .ok_or(CommandError::Usage(USAGE))
    }
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub target: Target,
    pub source: Frame3,
    pub options: Options,
    pub reference_length: Real,
    tolerance: Tolerance,
}
impl Placement {
    pub fn try_new(
        base: Point3,
        reference: Point3,
        target: Target,
        options: Options,
        plane: Frame3,
        tolerance: Tolerance,
    ) -> Result<Self, CommandError> {
        let (base, normal) = if options.on_surface {
            let l = target.location(base, options.ignore_trims)?;
            (l.point, target.source_normal(base, options.ignore_trims)?)
        } else {
            (
                base,
                options.source_normal.unwrap_or(plane.z_axis().as_vector()),
            )
        };
        let source = Frame3::try_from_x_and_normal(
            base,
            base.vector_to(reference)?,
            normal,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let reference_length = base.distance_to(reference)?;
        Ok(Self {
            target,
            source,
            options,
            reference_length,
            tolerance,
        })
    }
    pub fn locate(&self, point: Point3) -> Result<Pose, CommandError> {
        let l = self.target.location(point, self.options.ignore_trims)?;
        Ok(Pose {
            point: l.point,
            uv: l.uv,
            frame: l.surface.frame_at(l.uv[0], l.uv[1], self.tolerance)?,
        })
    }
    pub fn transform_at(&self, point: Point3) -> Result<AffineTransform3, CommandError> {
        self.transform_at_pose(self.locate(point)?, self.options)
    }
    pub fn transform_at_pose(
        &self,
        pose: Pose,
        options: Options,
    ) -> Result<AffineTransform3, CommandError> {
        let frame = target_frame(
            pose.frame,
            options.rotation.to_radians(),
            options.flip ^ self.target.reversed,
            self.tolerance,
        )?;
        Ok(AffineTransform3::try_frame_mapping(
            self.source,
            frame,
            [options.scale; 3],
        )?)
    }
    pub fn morph_at(
        &self,
        point: Point3,
        normal: Vector3,
    ) -> Result<SurfacePointMorph<'static>, CommandError> {
        self.morph_at_pose(self.locate(point)?, self.options, normal)
    }
    pub fn morph_at_pose(
        &self,
        pose: Pose,
        options: Options,
        normal: Vector3,
    ) -> Result<SurfacePointMorph<'static>, CommandError> {
        let mut morph = SurfacePointMorph::try_new_owned(
            self.source,
            self.target.surface.clone(),
            pose.uv[0],
            pose.uv[1],
            options.scale,
            options.rotation.to_radians(),
            options.flip ^ self.target.reversed,
            self.tolerance,
        )?;
        morph = morph.with_surface_orientation_policy(
            options
                .constrain_normal
                .then_some(options.destination_normal.unwrap_or(normal)),
        )?;
        if let Some(solver) = morph_interpolation_solver() {
            morph = morph.with_interpolation_solver(solver);
        }
        Ok(morph)
    }
    pub fn rotation_plane(&self, pose: Pose) -> Result<Frame3, GeometryError> {
        target_frame(
            pose.frame,
            0.,
            self.options.flip ^ self.target.reversed,
            self.tolerance,
        )
    }
    pub fn preview_pose(
        &self,
        phase: PreviewPhase,
        cursor: Point3,
    ) -> Result<(Pose, Options), CommandError> {
        let mut options = self.options;
        let pose = match phase {
            PreviewPhase::Target => self.locate(cursor)?,
            PreviewPhase::Scale(pose) => {
                options.scale = pose.point.distance_to(cursor)? / self.reference_length;
                if !options.scale.is_finite() || options.scale <= 0. {
                    return Err(CommandError::InvalidScaleFactor(options.scale.to_string()));
                }
                pose
            }
            PreviewPhase::Rotation(pose) => {
                let frame = self.rotation_plane(pose)?;
                let delta = pose.point.vector_to(cursor)?;
                let x = delta.dot(frame.x_axis().as_vector())?;
                let y = delta.dot(frame.y_axis().as_vector())?;
                if x == 0. && y == 0. {
                    return Err(CommandError::Usage(
                        "Pick a rotation direction away from the target point",
                    ));
                }
                options.rotation = y.atan2(x).to_degrees();
                pose
            }
        };
        Ok((pose, options))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub point: Point3,
    pub uv: [Real; 2],
    pub frame: Frame3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewPhase {
    Target,
    Scale(Pose),
    Rotation(Pose),
}

fn target_frame(
    frame: Frame3,
    angle: Real,
    flip: bool,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let (sin, cos) = angle.sin_cos();
    let sign = if flip { -1. } else { 1. };
    let x = combine_frame_axes(frame.x_axis(), frame.y_axis(), cos, sin * sign)?;
    let y = combine_frame_axes(frame.x_axis(), frame.y_axis(), -sin, cos * sign)?;
    Frame3::try_from_directions(frame.origin(), x, y, tolerance)
}
fn combine_frame_axes(
    first: UnitVector3,
    second: UnitVector3,
    a: Real,
    b: Real,
) -> Result<Vector3, GeometryError> {
    let f = first.as_vector().to_array();
    let s = second.as_vector().to_array();
    Vector3::try_from(std::array::from_fn(|i| a.mul_add(f[i], b * s[i])))
}
pub(super) struct OrientSurfaceCommand;
impl Command for OrientSurfaceCommand {
    fn name(&self) -> &'static str {
        "OrientOnSrf"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["OrientOnSurface"]
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(true)
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
        let (args, mut sources) = transform_arguments(doc, args, USAGE)?;
        let (args, options, reference) = options(&args, Options::default())?;
        if options.rigid && options.constrain_normal {
            return Err(CommandError::Usage(USAGE));
        }
        let (base, n) = parse_point(&args)?;
        let (second, m) = parse_point(&args[n..])?;
        let (point, k) = parse_point(&args[n + m..])?;
        require_consumed(&args, n + m + k, USAGE)?;
        let id = if let Some(id) = reference.id {
            id
        } else if let Some(name) = reference.name {
            let ids = doc
                .objects()
                .filter(|o| {
                    o.attributes()
                        .name()
                        .is_some_and(|v| v.eq_ignore_ascii_case(&name))
                })
                .map(|o| o.id())
                .collect::<Vec<_>>();
            match ids.as_slice() {
                [id] => *id,
                [] => return Err(CommandError::SurfaceOrientTargetNotFound(name)),
                _ => return Err(CommandError::AmbiguousSurfaceOrientTarget(name)),
            }
        } else {
            *sources
                .ids
                .last()
                .ok_or(CommandError::SurfaceOrientTargetRequired)?
        };
        sources.ids.retain(|s| *s != id);
        if sources.ids.is_empty() {
            return Err(CommandError::SurfaceOrientSourcesRequired);
        }
        let target = Target::capture(doc, id, reference.face)?;
        let placement = Placement::try_new(
            base,
            second,
            target,
            options,
            context.construction_plane,
            doc.tolerance(),
        )?;
        let uv = placement.target.location(point, options.ignore_trims)?.uv;
        let (transformed, copied) = if options.rigid {
            apply_transform_with_renewal(
                doc,
                &sources,
                placement.transform_at(point)?,
                options.copy,
            )?
        } else {
            let morph =
                placement.morph_at(point, context.construction_plane.z_axis().as_vector())?;
            let count = sources.ids.len();
            if options.copy {
                (
                    count,
                    doc.copy_objects_morphed(sources.ids.iter().copied(), &morph)?
                        .len(),
                )
            } else {
                (doc.morph_objects(sources.ids.iter().copied(), &morph)?, 0)
            }
        };
        doc.select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)?;
        Ok(format!(
            "Oriented {transformed} object(s) onto surface parameters {:.6},{:.6} with {} placement, creating {copied} copy object(s)",
            uv[0],
            uv[1],
            if options.rigid { "rigid" } else { "deformable" }
        ))
    }
}
