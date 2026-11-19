//! Tapered curve extrusion: kernel geometry, staged document admission.
use super::*;
pub use crate::extrude_surface::Distance;
use crate::extrude_surface::parse_face_extrusion_distance;

pub const USAGE: &str = "ExtrudeCrvTapered distance | base target [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle]";
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaperedExtrusionOptions {
    pub draft_angle: f64,
    pub solid: bool,
    pub round_corners: bool,
    pub delete_input: bool,
    pub direction: Option<Vector3>,
}
impl Default for TaperedExtrusionOptions {
    fn default() -> Self {
        Self {
            draft_angle: 5.,
            solid: false,
            round_corners: false,
            delete_input: false,
            direction: None,
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
            || ["Solid", "DeleteInput", "FlipAngle"]
                .iter()
                .any(|key| word.trim_start_matches('_').eq_ignore_ascii_case(key))
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
                "corners" => {
                    next.round_corners = match required()?
                        .trim_start_matches('_')
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "sharp" => false,
                        "round" => true,
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
    pub fn command_options(self) -> String {
        let mut result = format!(
            "DraftAngle={} Solid={} Corners={} DeleteInput={}",
            self.draft_angle,
            if self.solid { "Yes" } else { "No" },
            if self.round_corners { "Round" } else { "Sharp" },
            if self.delete_input { "Yes" } else { "No" }
        );
        if let Some(direction) = self.direction {
            let [x, y, z] = direction.to_array();
            result.push_str(&format!(" Direction={x},{y},{z}"));
        }
        result
    }
}

impl Command for ExtrudeTaperedCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvTapered"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args.iter().any(|s| !TaperedExtrusionOptions::recognizes(s)) {
            return Ok(None);
        }
        TaperedExtrusionOptions::parse_with_options(args, self.settings.get().options)?;
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
        let (options, distance) = parse(args, self.settings.get().options)?;
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
fn parse(
    args: &[&str],
    defaults: TaperedExtrusionOptions,
) -> Result<(TaperedExtrusionOptions, Distance), CommandError> {
    let (options, positional): (Vec<_>, Vec<_>) = args
        .iter()
        .copied()
        .partition(|s| TaperedExtrusionOptions::recognizes(s));
    let options = TaperedExtrusionOptions::parse_with_options(&options, defaults)?;
    let distance = parse_face_extrusion_distance(&positional, options.direction.is_some(), USAGE)?;
    Ok((options, distance))
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

fn local_profile(
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

fn profile_normal(
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
    use viboceros_smlib::Shape;
    let (origin, profile) = local_profile(curve)?;
    let (displacement, draft) = displacement_and_draft(distance, options, || {
        Ok(profile_normal(&profile, tolerance)?.unwrap_or(cplane.z_axis().as_vector()))
    })?;
    let cap = options.solid && curve.is_closed()?;
    let brep = Shape::tapered_extrusion(
        &profile,
        displacement,
        draft,
        options.round_corners,
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
#[cfg(test)]
mod tests;
