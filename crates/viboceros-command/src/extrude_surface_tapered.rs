//! Tapered surface extrusion with original face selection and staged admission.
use super::*;
use crate::extrude_surface::{
    Distance, SurfaceExtrusionInput, SurfaceFaces, apply_staged_products, capture_source_face,
    parse_face_extrusion_distance, surface_extrusion_targets,
};
use crate::extrude_tapered::{
    TaperedExtrusionOptions, TaperedExtrusionSettings, retain_selected_inputs,
};
use crate::surface_reference::BoundaryInput;

pub const USAGE: &str = "ExtrudeSrfTapered distance | base target | ToBoundary=id[:face] [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round|Smooth] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceTaperedCommand {
    pub(super) settings: std::sync::Arc<remembered::Remembered<TaperedExtrusionSettings>>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceTaperedInput {
    pub options: TaperedExtrusionOptions,
    pub faces: SurfaceFaces,
    pub boundary: BoundaryInput,
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
            let key = word
                .trim_start_matches('_')
                .split('=')
                .next()
                .unwrap_or_default();
            if key.eq_ignore_ascii_case("SplitAtTangents") {
                return Err(CommandError::Usage(USAGE));
            }
            if ["Face", "Faces", "FaceRefs"]
                .iter()
                .any(|s| key.eq_ignore_ascii_case(s))
            {
                face_words.push(*word);
            } else {
                common.push(*word);
            }
        }
        let (options, boundary) = TaperedExtrusionOptions::parse_command_options(&common, options)
            .map_err(|error| match error {
                CommandError::Usage(_) => CommandError::Usage(USAGE),
                error => error,
            })?;
        Ok(Self {
            faces: SurfaceExtrusionInput::parse(&face_words)?.faces,
            options,
            boundary,
        })
    }
}
impl Command for ExtrudeSurfaceTaperedCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrfTapered"
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
        let settings = self.settings.get();
        let input = SurfaceTaperedInput::parse_with_options(&words, settings.options)?;
        match input.boundary {
            BoundaryInput::None => {}
            BoundaryInput::Select => return Err(CommandError::Usage("Select a boundary surface")),
            BoundaryInput::Reference(reference) => {
                if !positional.is_empty() {
                    return Err(CommandError::Usage(USAGE));
                }
                let options = settings.boundary_options(input.options);
                let count = crate::tapered_boundary::apply(
                    doc,
                    crate::tapered_boundary::Inputs::Faces(&input.faces),
                    options,
                    reference,
                    context.construction_plane,
                )?;
                self.settings.set(TaperedExtrusionSettings::after_success(
                    options,
                    settings.distance,
                ));
                return Ok(format!("Tapered {count} surface face(s) to the boundary"));
            }
        }
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
            options.corners.kernel(),
            options.solid,
            tolerance,
        )?
        .to_brep(tolerance)?
        .transformed(restore, tolerance)?)
}

#[cfg(test)]
mod tests;
