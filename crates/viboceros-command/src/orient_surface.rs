//! Shared rigid/splop placement on a captured surface reference.
use super::*;
use std::sync::Arc;
use viboceros_document::GeometrySnapshot;
pub const USAGE: &str = "OrientOnSrf base reference target [Surface=id] [Face=index] [SurfaceName=name] [Scale=factor|Prompt|Uniform|XYZ] [ScaleXYZ=x,y,z] [X=factor] [Y=factor] [Z=factor] [ScalePrompt=Yes|No] [Rotation=degrees|Prompt|Angle] [RotationPrompt=Yes|No] [Rigid=Yes|No] [Copy=Yes|No] [Flip=Yes|No] [ConstrainNormal=Yes|No] [SourceNormal=x,y,z] [DestinationNormal=x,y,z] [OnSurface=Yes|No] [IgnoreTrims=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub scale: Real,
    pub axes: Option<[Real; 3]>,
    pub scale_prompt: bool,
    pub rotation_prompt: bool,
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
            axes: None,
            scale_prompt: false,
            rotation_prompt: false,
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
    pub fn scales(self) -> [Real; 3] {
        if self.scale_prompt {
            [self.scale; 3]
        } else {
            self.axes.unwrap_or([self.scale; 3])
        }
    }
    pub fn preferences(mut self) -> Self {
        self.copy = true;
        self.on_surface = false;
        self.source_normal = None;
        self.destination_normal = None;
        if self.rigid {
            self.constrain_normal = false;
        }
        self
    }
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
        if let Some([x, y, z]) = self.axes {
            args += &format!(" ScaleXYZ={x},{y},{z}");
        }
        args += &format!(
            " ScalePrompt={} RotationPrompt={}",
            yes(self.scale_prompt),
            yes(self.rotation_prompt)
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
            "uniform" => "scale",
            "scalex" => "x",
            "scaley" => "y",
            "scalez" => "z",
            "up" => "sourcenormal",
            _ => &key,
        };
        if [
            "scale",
            "rotation",
            "angle",
            "scalexyz",
            "scaleprompt",
            "rotationprompt",
            "x",
            "y",
            "z",
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
                "scale" => match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                    "prompt" => options.scale_prompt = true,
                    "uniform" => {
                        options.scale_prompt = false;
                        options.axes = None;
                    }
                    "xyz" => {
                        options.axes = Some(options.axes.unwrap_or([options.scale; 3]));
                        options.scale_prompt = false;
                    }
                    _ if value.contains(',') => {
                        options.axes = Some(parse_axes(value)?);
                        options.scale_prompt = false;
                    }
                    _ => {
                        options.scale = parse_finite_real(value)?;
                        if options.scale <= 0. {
                            return Err(CommandError::InvalidScaleFactor(value.into()));
                        }
                        options.axes = None;
                        options.scale_prompt = false;
                    }
                },
                "scalexyz" => {
                    options.axes = Some(parse_axes(value)?);
                    options.scale_prompt = false;
                }
                "x" | "y" | "z" => {
                    let mut axes = options.axes.unwrap_or([options.scale; 3]);
                    axes[match key {
                        "x" => 0,
                        "y" => 1,
                        _ => 2,
                    }] = parse_finite_real(value)?;
                    options.axes = Some(axes);
                    options.scale_prompt = false;
                }
                "scaleprompt" => options.scale_prompt = toggle()?,
                "rotationprompt" => options.rotation_prompt = toggle()?,
                "rotation" | "angle" => {
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "prompt" if key == "rotation" => options.rotation_prompt = true,
                        "angle" if key == "rotation" => options.rotation_prompt = false,
                        _ => {
                            options.rotation = parse_finite_real(value)?;
                            if !options.rotation.to_radians().is_finite() {
                                return Err(CommandError::InvalidNumber(value.into()));
                            }
                            options.rotation_prompt = false;
                        }
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
    if options.rigid && !seen.contains("constrainnormal") {
        options.constrain_normal = false;
    }
    if options.axes.is_some_and(|axes| axes.contains(&0.)) {
        return Err(CommandError::InvalidScaleFactor(
            "axis scale cannot be zero".into(),
        ));
    }
    Ok((rest, options, reference))
}
fn parse_axes(value: &str) -> Result<[Real; 3], CommandError> {
    let parts = value
        .split(',')
        .map(parse_finite_real)
        .collect::<Result<Vec<_>, _>>()?;
    parts.try_into().map_err(|_| CommandError::Usage(USAGE))
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
            options.scales(),
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
            if !options.scale_prompt && options.axes.is_some() {
                1.
            } else {
                options.scale
            },
            options.rotation.to_radians(),
            options.flip ^ self.target.reversed,
            self.tolerance,
        )?;
        if !options.scale_prompt
            && let Some(axes) = options.axes
        {
            morph = morph.with_source_axis_scales(axes)?;
        }
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
#[derive(Default)]
pub(super) struct OrientSurfaceCommand(remembered::Remembered<Options>);
impl Command for OrientSurfaceCommand {
    fn name(&self) -> &'static str {
        "OrientOnSrf"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["OrientOnSurface"]
    }
    fn orient_surface_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_orient_surface_options(&self, value: Options) -> bool {
        self.0.set(value.preferences());
        true
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
        let (args, mut sources) = affine_transform_arguments(doc, args, USAGE)?;
        let (args, options, reference) = options(&args, self.0.get())?;
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
        if sources.grips.iter().any(|grip| grip.object == id) {
            return Err(CommandError::Usage(
                "Destination surface cannot also supply source grips",
            ));
        }

        if sources.ids.is_empty() && sources.grips.is_empty() {
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
        let (transformed, copied) = if options.rigid && !sources.grips.is_empty() {
            let (owners, copies) = doc.transform_objects_and_grips(
                sources.ids.iter().copied(),
                sources.grips.iter().copied(),
                placement.transform_at(point)?,
                options.copy,
                viboceros_document::CopyGroupPolicy::Preserve,
            )?;
            // Native copies one ordinary peer without its memberships, while
            // the copied grip owners keep their independent copied groups.
            let ordinary = owners
                .iter()
                .filter(|id| !sources.grips.iter().any(|g| g.object == **id))
                .copied()
                .collect::<Vec<_>>();
            if options.copy && ordinary.len() == 1 {
                let copy = copies[owners.iter().position(|id| *id == ordinary[0]).unwrap()];
                doc.clear_object_group_memberships([copy])?;
            }
            if options.copy && sources.postselected {
                doc.select_control_points(
                    sources.grips.iter().copied(),
                    viboceros_document::SelectionMode::Remove,
                )?;
            }
            (owners.len(), copies.len())
        } else if options.rigid {
            apply_transform_with_renewal(
                doc,
                &sources,
                placement.transform_at(point)?,
                options.copy,
            )?
        } else {
            let morph =
                placement.morph_at(point, context.construction_plane.z_axis().as_vector())?;
            if !sources.grips.is_empty() {
                let (owners, copies) = doc.morph_objects_and_grips(
                    sources.ids.iter().copied(),
                    sources.grips.iter().copied(),
                    &morph,
                    options.copy,
                    viboceros_document::CopyGroupPolicy::Preserve,
                )?;
                (owners.len(), copies.len())
            } else {
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
            }
        };
        doc.select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)?;
        if sources.postselected && !sources.grips.is_empty() {
            if options.rigid {
                doc.select_command_results([])?;
            } else {
                doc.clear_selection();
            }
        }

        self.0.set(options.preferences());
        Ok(format!(
            "Oriented {transformed} object(s) onto surface parameters {:.6},{:.6} with {} placement, creating {copied} copy object(s)",
            uv[0],
            uv[1],
            if options.rigid { "rigid" } else { "deformable" }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_surface_presets_roundtrip_modes_axes_and_reject_invalid_values() {
        let initial = Options {
            axes: Some([-2., 3., 4.]),
            scale_prompt: true,
            rotation_prompt: true,
            rotation: -45.,
            ..Default::default()
        };
        let args = initial.arguments();
        let (_, parsed, _) = options(
            &args.split_whitespace().collect::<Vec<_>>(),
            Options::default(),
        )
        .unwrap();
        assert_eq!(initial, parsed);
        assert_eq!(parsed.scales(), [1.; 3]);
        let (_, xyz, _) = options(&["ScalePrompt=No"], parsed).unwrap();
        assert_eq!(xyz.scales(), [-2., 3., 4.]);
        let (_, uniform, _) = options(&["Uniform=2"], xyz).unwrap();
        assert_eq!(uniform.scales(), [2.; 3]);
        assert!(uniform.axes.is_none());
        let remembered = Options {
            rigid: false,
            constrain_normal: true,
            ..uniform
        };
        let (_, rigid, _) = options(&["Rigid=Yes"], remembered).unwrap();
        assert!(!rigid.constrain_normal);
        for args in [
            vec!["X=0"],
            vec!["ScaleXYZ=1,2,0"],
            vec!["ScaleXYZ=1,2"],
            vec!["ScaleXYZ=1,NaN,3"],
            vec!["Angle=inf"],
            vec!["ScalePrompt=Maybe"],
            vec!["X=2", "ScaleX=3"],
        ] {
            assert!(options(&args, initial).is_err(), "{args:?}");
        }
    }
    #[test]
    fn orient_surface_preferences_are_registry_scoped_and_failed_cli_is_atomic() {
        let registry = CommandRegistry::with_builtins();
        let desired = Options {
            axes: Some([-2., 3., 4.]),
            rotation: 30.,
            rigid: false,
            copy: false,
            source_normal: Some(Vector3::try_new(0., 0., 1.).unwrap()),
            ..Default::default()
        };
        assert!(registry.remember_orient_surface_options(desired));
        assert_eq!(registry.orient_surface_options(), desired.preferences());
        assert_eq!(
            CommandRegistry::with_builtins().orient_surface_options(),
            Options::default()
        );
        let mut doc = Document::default();
        registry.execute(&mut doc, "Point 0,0,0").unwrap();
        let before = format!("{doc:?}");
        for args in [
            "ScaleXYZ=2,0,4",
            "X=NaN",
            "Rotation=inf",
            "ScaleXYZ=2,3,4 SurfaceName=Missing",
        ] {
            assert!(
                registry
                    .execute(
                        &mut doc,
                        &format!("OrientOnSurface 0,0,0 1,0,0 2,3,4 {args}")
                    )
                    .is_err()
            );
            assert_eq!(registry.orient_surface_options(), desired.preferences());
            assert_eq!(format!("{doc:?}"), before);
        }
        let invalid = Options {
            axes: Some([2., 0., 4.]),
            ..desired
        };
        assert!(!registry.remember_orient_surface_options(invalid));
        assert_eq!(registry.orient_surface_options(), desired.preferences());
    }
}
