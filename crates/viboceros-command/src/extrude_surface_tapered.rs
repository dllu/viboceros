//! Tapered surface extrusion with original face selection and staged admission.
use super::*;
use crate::extrude_surface::{
    Distance, SurfaceExtrusionInput, SurfaceFaces, apply_face_extrusions,
    parse_face_extrusion_distance,
};
use crate::extrude_tapered::TaperedExtrusionOptions;
pub const USAGE: &str = "ExtrudeSrfTapered distance | base target [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceTaperedCommand;
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTaperedInput {
    pub options: TaperedExtrusionOptions,
    pub faces: SurfaceFaces,
}
impl Default for SurfaceTaperedInput {
    fn default() -> Self {
        Self {
            options: TaperedExtrusionOptions {
                solid: true,
                ..Default::default()
            },
            faces: SurfaceFaces::default(),
        }
    }
}
impl SurfaceTaperedInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
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
            ..Self::default()
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
        if args.iter().any(|s| {
            !s.contains('=') && !s.trim_start_matches('_').eq_ignore_ascii_case("FlipAngle")
        }) {
            return Ok(None);
        }
        let input = SurfaceTaperedInput::parse(args)?;
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
        let (positional, words): (Vec<_>, Vec<_>) = args.iter().copied().partition(|s| {
            !s.contains('=') && !s.trim_start_matches('_').eq_ignore_ascii_case("FlipAngle")
        });
        let input = SurfaceTaperedInput::parse(&words)?;
        let distance =
            parse_face_extrusion_distance(&positional, input.options.direction.is_some(), USAGE)?;
        let count = apply_face_extrusions(
            doc,
            &input.faces,
            input.options.delete_input,
            |brep, tolerance| {
                #[cfg(feature = "native-smlib")]
                {
                    let origin = brep
                        .vertices()
                        .first()
                        .map(|v| v.point())
                        .unwrap_or_else(|| brep.bounds().min());
                    let zero = Point3::try_new(0., 0., 0.)?;
                    let local = brep.transformed(
                        AffineTransform3::from_translation(origin.vector_to(zero)?),
                        tolerance,
                    )?;
                    let shape = viboceros_smlib::Shape::from_brep(&local, tolerance)?;
                    let displacement = match distance {
                        Distance::Explicit(v) => v,
                        Distance::Numeric(h) => crate::extrude_surface::face_extrusion_direction(
                            &shape,
                            input.options.direction,
                            context.construction_plane,
                            tolerance,
                        )?
                        .scaled(h)?,
                    };
                    let b = shape
                        .tapered_face(
                            displacement,
                            input.options.draft_angle,
                            input.options.round_corners,
                            input.options.solid,
                            tolerance,
                        )?
                        .to_brep(tolerance)?
                        .transformed(
                            AffineTransform3::from_translation(zero.vector_to(origin)?),
                            tolerance,
                        )?;
                    Ok(Geometry::Brep(b))
                }
                #[cfg(not(feature = "native-smlib"))]
                {
                    match distance {
                        Distance::Explicit(v) => {
                            let _ = v;
                        }
                        Distance::Numeric(h) => {
                            let _ = h;
                        }
                    }
                    let _ = (brep, tolerance, context);
                    Err(CommandError::SurfaceTaperedExtrusionRequiresNativeKernel)
                }
            },
        )?;
        Ok(format!("Tapered {count} surface face(s)"))
    }
}
#[cfg(test)]
mod tests;
