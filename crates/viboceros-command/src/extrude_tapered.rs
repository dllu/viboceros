//! Tapered curve extrusion: kernel geometry, staged document admission.
use super::*;
pub use crate::extrude_surface::Distance;
use crate::extrude_surface::parse_face_extrusion_distance;
use crate::surface_reference::BoundaryInput;

mod partitions;

pub const USAGE: &str = "ExtrudeCrvTapered distance | base target | ToBoundary=id[:face] [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round|Smooth] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle] [SplitAtTangents=Yes|No]";
pub(super) struct ExtrudeTaperedCommand {
    pub(super) settings: std::sync::Arc<remembered::Remembered<TaperedExtrusionSettings>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaperedExtrusionSettings {
    pub options: TaperedExtrusionOptions,
    pub distance: f64,
}

impl Default for TaperedExtrusionSettings {
    fn default() -> Self {
        Self {
            options: TaperedExtrusionOptions::default(),
            distance: 1.,
        }
    }
}

impl TaperedExtrusionSettings {
    pub(super) fn boundary_options(
        self,
        mut options: TaperedExtrusionOptions,
    ) -> TaperedExtrusionOptions {
        if options.draft_angle == 0. {
            options.draft_angle = if self.options.draft_angle == 0. {
                TaperedExtrusionOptions::default().draft_angle
            } else {
                self.options.draft_angle
            };
        }
        options
    }
    pub(super) fn after_success(options: TaperedExtrusionOptions, distance: f64) -> Self {
        Self {
            options: TaperedExtrusionOptions {
                direction: None,
                ..options
            },
            distance,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaperCorners {
    #[default]
    Sharp,
    Round,
    Smooth,
}

impl TaperCorners {
    fn name(self) -> &'static str {
        match self {
            Self::Sharp => "Sharp",
            Self::Round => "Round",
            Self::Smooth => "Smooth",
        }
    }

    pub(super) fn kernel(self) -> viboceros_smlib::TaperCorner {
        use viboceros_smlib::TaperCorner;

        match self {
            Self::Sharp => TaperCorner::Sharp,
            Self::Round => TaperCorner::Round,
            Self::Smooth => TaperCorner::Smooth,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaperedExtrusionOptions {
    pub draft_angle: f64,
    pub solid: bool,
    pub corners: TaperCorners,
    pub delete_input: bool,
    pub direction: Option<Vector3>,
    pub split_at_tangents: bool,
}
impl Default for TaperedExtrusionOptions {
    fn default() -> Self {
        Self {
            draft_angle: 5.,
            solid: false,
            corners: TaperCorners::Sharp,
            delete_input: false,
            direction: None,
            split_at_tangents: false,
        }
    }
}
impl TaperedExtrusionOptions {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_options(words, Self::default())
    }
    pub fn parse_with_options(words: &[&str], mut result: Self) -> Result<Self, CommandError> {
        result.update(words)?;
        Ok(result)
    }

    pub fn recognizes(word: &str) -> bool {
        word.contains('=')
            || [
                "Solid",
                "DeleteInput",
                "FlipAngle",
                "ToBoundary",
                "SplitAtTangents",
            ]
            .iter()
            .any(|key| word.trim_start_matches('_').eq_ignore_ascii_case(key))
    }

    /// Parse the options accepted before a distance or a cutter pick.
    pub fn parse_command_options(
        words: &[&str],
        defaults: Self,
    ) -> Result<(Self, BoundaryInput), CommandError> {
        let mut boundary = BoundaryInput::None;
        let mut options = Vec::with_capacity(words.len());
        for &word in words {
            if let Some(parsed) = BoundaryInput::parse_option(word)? {
                if boundary != BoundaryInput::None {
                    return Err(CommandError::Usage(USAGE));
                }
                boundary = parsed;
            } else {
                options.push(word);
            }
        }
        Ok((Self::parse_with_options(&options, defaults)?, boundary))
    }
    /// Apply one accepted option line atomically, retaining prior values.
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let word = word.trim_start_matches('_');
            if word.eq_ignore_ascii_case("FlipAngle") {
                if !seen.insert("flipangle".into()) {
                    return Err(CommandError::Usage(USAGE));
                }
                next.draft_angle = -next.draft_angle;
                continue;
            }
            let (key, value) = word
                .split_once('=')
                .map_or((word, None), |(key, value)| (key, Some(value)));
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            let required = || value.ok_or(CommandError::Usage(USAGE));
            let toggle = |current: bool| {
                value.map_or(Ok(!current), |value| {
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))
                })
            };
            match key.as_str() {
                "draftangle" => {
                    let angle = parse_finite_real(required()?)?;
                    if angle.abs() >= 90. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.draft_angle = angle;
                }
                "solid" => next.solid = toggle(next.solid)?,
                "deleteinput" => next.delete_input = toggle(next.delete_input)?,
                "splitattangents" => next.split_at_tangents = toggle(next.split_at_tangents)?,
                "corners" => {
                    next.corners = match required()?
                        .trim_start_matches('_')
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "sharp" => TaperCorners::Sharp,
                        "round" => TaperCorners::Round,
                        "smooth" => TaperCorners::Smooth,
                        _ => return Err(CommandError::Usage(USAGE)),
                    }
                }
                "direction" => {
                    let (point, count) = parse_point(&[required()?])?;
                    if count != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.direction = Some(
                        Vector3::try_from(point.to_array())?
                            .normalized_nonzero()?
                            .as_vector(),
                    );
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self, surface: bool) -> String {
        let mut result = format!(
            "DraftAngle={} Solid={} Corners={} DeleteInput={}",
            self.draft_angle,
            if self.solid { "Yes" } else { "No" },
            self.corners.name(),
            if self.delete_input { "Yes" } else { "No" }
        );
        if let Some(direction) = self.direction {
            let [x, y, z] = direction.to_array();
            result.push_str(&format!(" Direction={x},{y},{z}"));
        }
        if !surface {
            let value = if self.split_at_tangents { "Yes" } else { "No" };
            result.push_str(&format!(" SplitAtTangents={value}"));
        }
        result
    }
}

impl Command for ExtrudeTaperedCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvTapered"
    }
    fn commits_on_error(&self, error: &CommandError) -> bool {
        matches!(error, CommandError::SurfaceExtrusionBoundaryRejected)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args.iter().any(|s| !TaperedExtrusionOptions::recognizes(s)) {
            return Ok(None);
        }
        TaperedExtrusionOptions::parse_command_options(args, self.settings.get().options)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
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
        let settings = self.settings.get();
        let (words, positional): (Vec<_>, Vec<_>) = args
            .iter()
            .copied()
            .partition(|s| TaperedExtrusionOptions::recognizes(s));
        let (options, boundary) =
            TaperedExtrusionOptions::parse_command_options(&words, settings.options)?;
        match boundary {
            BoundaryInput::None => {}
            BoundaryInput::Select => return Err(CommandError::Usage("Select a boundary surface")),
            BoundaryInput::Reference(reference) => {
                if !positional.is_empty() {
                    return Err(CommandError::Usage(USAGE));
                }
                let options = settings.boundary_options(options);
                let count = crate::tapered_boundary::apply(
                    doc,
                    crate::tapered_boundary::Inputs::Curves,
                    options,
                    reference,
                    context.construction_plane,
                )?;
                self.settings.set(TaperedExtrusionSettings::after_success(
                    options,
                    settings.distance,
                ));
                return Ok(format!("Tapered {count} curve object(s) to the boundary"));
            }
        }
        let distance =
            parse_face_extrusion_distance(&positional, options.direction.is_some(), USAGE)?;
        let curves = doc
            .selected_objects()
            .filter_map(|o| {
                o.geometry()
                    .nurbs_curve_representation()
                    .map(|c| c.map(|c| (o.id(), c)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if curves.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }
        let mut staged = Vec::new();
        for (id, curve) in curves {
            staged.push((
                id,
                Geometry::Brep(produce_curve(
                    &curve,
                    distance,
                    options,
                    context.construction_plane,
                    doc.tolerance(),
                )?),
            ));
        }
        let count = staged.len();
        let selection = doc.selected_object_ids().collect::<Vec<_>>();
        if !options.delete_input {
            doc.release_command_selection_on_history_replay(staged.iter().map(|(id, _)| *id))?;
        }
        for (source, geometry) in staged {
            doc.add_geometry(geometry)?;
            if options.delete_input {
                doc.delete_object(source)?;
            }
        }
        retain_selected_inputs(doc, selection)?;
        self.settings.set(TaperedExtrusionSettings::after_success(
            options,
            distance.remembered()?,
        ));
        Ok(format!("Tapered {count} curve object(s)"))
    }
}
impl Distance {
    pub(super) fn remembered(self) -> Result<f64, GeometryError> {
        match self {
            Self::Numeric(height) => Ok(height),
            Self::Explicit(displacement) => displacement.length(),
        }
    }
}

pub(super) fn displacement_and_draft(
    distance: Distance,
    options: TaperedExtrusionOptions,
    reference: impl FnOnce() -> Result<Vector3, CommandError>,
) -> Result<(Vector3, f64), CommandError> {
    match distance {
        Distance::Explicit(displacement) => Ok((displacement, options.draft_angle)),
        Distance::Numeric(height) => {
            let reference = reference()?;
            let direction = options.direction.unwrap_or(reference);
            let draft = if direction.dot(reference)? < 0. {
                -options.draft_angle
            } else {
                options.draft_angle
            };
            Ok((direction.scaled(height)?, draft))
        }
    }
}

pub(super) fn retain_selected_inputs(
    doc: &mut Document,
    selection: Vec<ObjectId>,
) -> Result<(), DocumentError> {
    let retained = selection
        .into_iter()
        .filter(|&id| doc.object(id).is_some())
        .collect::<Vec<_>>();
    doc.select_command_results(retained)?;
    Ok(())
}

pub(super) fn local_profile(
    curve: &NurbsCurve,
) -> Result<(Point3, viboceros_smlib::KernelCurve), CommandError> {
    let origin = curve.evaluate(*curve.domain().start())?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let local = curve.transformed(AffineTransform3::from_translation(origin.vector_to(zero)?))?;
    Ok((
        origin,
        viboceros_smlib::KernelCurve::from_nurbs_for_modeling(&local)?,
    ))
}

pub(super) fn profile_normal(
    profile: &viboceros_smlib::KernelCurve,
    tolerance: Tolerance,
) -> Result<Option<Vector3>, CommandError> {
    Ok(
        viboceros_smlib::KernelCurve::parallel_plane_normal(&[profile], tolerance)?
            .map(crate::extrude_surface::oriented_plane_normal)
            .transpose()?,
    )
}

/// Reference direction for the numeric distance getter.
pub fn curve_reference_normal(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<Vector3>, CommandError> {
    let (_, profile) = local_profile(curve)?;
    profile_normal(&profile, tolerance)
}

/// Construct the same model-space sweep for document edits and interactive previews.
pub fn produce_curve(
    curve: &NurbsCurve,
    distance: Distance,
    options: TaperedExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let (origin, profile) = local_profile(curve)?;
    let (displacement, draft) = displacement_and_draft(distance, options, || {
        Ok(profile_normal(&profile, tolerance)?.unwrap_or(cplane.z_axis().as_vector()))
    })?;
    let cap = options.solid && curve.is_closed()?;
    let brep = produce_curve_shape(
        curve,
        &profile,
        displacement,
        draft,
        options,
        cap,
        tolerance,
    )?
    .to_brep(tolerance)?
    .transformed(
        AffineTransform3::from_translation(Point3::try_new(0., 0., 0.)?.vector_to(origin)?),
        tolerance,
    )?;
    if cap && !brep.is_solid() {
        return Err(CommandError::Usage(
            "Tapered extrusion did not produce a closed solid",
        ));
    }
    Ok(brep)
}

pub(super) fn produce_curve_shape(
    source: &NurbsCurve,
    profile: &viboceros_smlib::KernelCurve,
    displacement: Vector3,
    draft: f64,
    options: TaperedExtrusionOptions,
    capped: bool,
    tolerance: Tolerance,
) -> Result<viboceros_smlib::Shape, viboceros_smlib::Error> {
    use viboceros_smlib::Shape;

    if draft == 0. {
        return Shape::linear_sweep(&[profile], displacement, capped, tolerance);
    }
    let cuts = partitions::for_sweep(
        source,
        displacement,
        draft,
        options.split_at_tangents,
        tolerance,
    )?;
    Shape::tapered_extrusion_partitioned(
        profile,
        displacement,
        draft,
        options.corners.kernel(),
        capped,
        tolerance,
        &cuts,
    )
}
#[cfg(test)]
mod tests;
