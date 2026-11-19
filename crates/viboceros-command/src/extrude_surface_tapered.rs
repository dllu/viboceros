//! Tapered surface extrusion with original face selection and staged admission.
use super::*;
use crate::extrude_surface::{
    Distance, SurfaceExtrusionInput, SurfaceFaces, apply_staged_products, capture_source_face,
    parse_face_extrusion_distance, surface_extrusion_targets,
};
use crate::extrude_tapered::{
    TaperedExtrusionOptions, TaperedExtrusionSettings, retain_selected_inputs,
};
pub const USAGE: &str = "ExtrudeSrfTapered distance | base target [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceTaperedCommand {
    pub(super) settings: std::sync::Arc<remembered::Remembered<TaperedExtrusionSettings>>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceTaperedInput {
    pub options: TaperedExtrusionOptions,
    pub faces: SurfaceFaces,
}
impl SurfaceTaperedInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_options(words, TaperedExtrusionOptions::default())
    }

    pub fn parse_with_options(
        words: &[&str],
        options: TaperedExtrusionOptions,
    ) -> Result<Self, CommandError> {
        let (mut face_words, mut common) = (Vec::new(), Vec::new());
        for word in words {
            if word
                .trim_start_matches('_')
                .split_once('=')
                .is_some_and(|(key, _)| {
                    ["Face", "Faces", "FaceRefs"]
                        .iter()
                        .any(|s| key.eq_ignore_ascii_case(s))
                })
            {
                face_words.push(*word);
            } else {
                common.push(*word);
            }
        }
        let mut result = Self {
            faces: SurfaceExtrusionInput::parse(&face_words)?.faces,
            options,
        };
        result.options.update(&common).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            e => e,
        })?;
        Ok(result)
    }
}
impl Command for ExtrudeSurfaceTaperedCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrfTapered"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args.iter().any(|s| !TaperedExtrusionOptions::recognizes(s)) {
            return Ok(None);
        }
        let input = SurfaceTaperedInput::parse_with_options(args, self.settings.get().options)?;
        if matches!(input.faces, SurfaceFaces::References(_)) {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::SurfaceComponents,
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
        let (words, positional): (Vec<_>, Vec<_>) = args
            .iter()
            .copied()
            .partition(|s| TaperedExtrusionOptions::recognizes(s));
        let input = SurfaceTaperedInput::parse_with_options(&words, self.settings.get().options)?;
        let distance =
            parse_face_extrusion_distance(&positional, input.options.direction.is_some(), USAGE)?;
        let targets = surface_extrusion_targets(doc, &input.faces)?;
        let count = targets.len();
        let tolerance = doc.tolerance();
        let mut products = Vec::with_capacity(count);
        for &(id, face) in &targets {
            let source = capture_source_face(doc, id, face)?;
            products.push(Geometry::Brep(produce_surface(
                &source,
                distance,
                input.options,
                context.construction_plane,
                tolerance,
            )?));
        }
        let selection = doc.selected_object_ids().collect::<Vec<_>>();
        if !input.options.delete_input {
            let objects = targets.iter().map(|&(id, _)| id).collect::<BTreeSet<_>>();
            doc.release_command_selection_on_history_replay(
                selection.iter().copied().filter(|id| objects.contains(id)),
            )?;
        }
        apply_staged_products(
            doc,
            products,
            targets.into_iter().filter(|_| input.options.delete_input),
        )?;
        retain_selected_inputs(doc, selection)?;
        self.settings.set(TaperedExtrusionSettings::after_success(
            input.options,
            distance.remembered()?,
        ));
        Ok(format!("Tapered {count} surface face(s)"))
    }
}
/// Construct the same one-face sweep for document edits and interactive previews.
pub fn produce_surface(
    brep: &Brep,
    distance: Distance,
    options: TaperedExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let (origin, local) = crate::extrude_surface::local_sweep_source(brep, tolerance)?;
    let shape = viboceros_smlib::Shape::from_brep(&local, tolerance)?;
    let (displacement, draft) =
        crate::extrude_tapered::displacement_and_draft(distance, options, || {
            crate::extrude_surface::face_extrusion_direction(&shape, None, cplane, tolerance)
        })?;
    let restore =
        AffineTransform3::from_translation(Point3::try_new(0., 0., 0.)?.vector_to(origin)?);
    Ok(shape
        .tapered_face(
            displacement,
            draft,
            options.round_corners,
            options.solid,
            tolerance,
        )?
        .to_brep(tolerance)?
        .transformed(restore, tolerance)?)
}

#[cfg(test)]
mod tests;
