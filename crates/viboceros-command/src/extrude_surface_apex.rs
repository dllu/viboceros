//! Native surface and original face extrusion to an explicit apex.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, apply_face_extrusion_products};
pub const USAGE: &str = "ExtrudeSrfToPoint apex [Solid=Yes|No] [DeleteInput=Yes|No] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceToPointCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceApexOptions {
    pub solid: bool,
    pub delete_input: bool,
}
impl Default for SurfaceApexOptions {
    fn default() -> Self {
        Self {
            solid: true,
            delete_input: false,
        }
    }
}
impl SurfaceApexOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            match key.as_str() {
                "solid" => next.solid = value,
                "deleteinput" => next.delete_input = value,
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Solid={} DeleteInput={}",
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" }
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceApexInput {
    pub options: SurfaceApexOptions,
    pub faces: SurfaceFaces,
    pub apex: Option<Point3>,
}
impl SurfaceApexInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        let (mut faces, mut options, mut positional) = (Vec::new(), Vec::new(), Vec::new());
        for word in words {
            if let Some((key, _)) = word.trim_start_matches('_').split_once('=') {
                if ["Face", "Faces", "FaceRefs"]
                    .iter()
                    .any(|s| key.eq_ignore_ascii_case(s))
                {
                    faces.push(*word);
                } else {
                    options.push(*word);
                }
            } else {
                positional.push(*word);
            }
        }
        let mut result = Self {
            faces: SurfaceExtrusionInput::parse(&faces)?.faces,
            ..Self::default()
        };
        result.options.update(&options)?;
        if !positional.is_empty() {
            let (apex, count) = parse_point(&positional)?;
            require_consumed(&positional, count, USAGE)?;
            result.apex = Some(apex);
        }
        Ok(result)
    }
}
impl Command for ExtrudeSurfaceToPointCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrfToPoint"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = SurfaceApexInput::parse(args)?;
        if input.apex.is_some() || matches!(input.faces, SurfaceFaces::References(_)) {
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
        let input = SurfaceApexInput::parse(args)?;
        let apex = input.apex.ok_or(CommandError::Usage(USAGE))?;
        let count = apply_face_extrusion_products(
            doc,
            &input.faces,
            input.options.delete_input,
            |brep, tolerance| {
                #[cfg(feature = "native-smlib")]
                {
                    let zero = Point3::try_new(0., 0., 0.)?;
                    let origin = brep
                        .vertices()
                        .first()
                        .map(|v| v.point())
                        .unwrap_or_else(|| brep.bounds().min());
                    let shift = origin.vector_to(zero)?;
                    let local =
                        brep.transformed(AffineTransform3::from_translation(shift), tolerance)?;
                    let result = viboceros_smlib::Shape::from_brep(&local, tolerance)?
                        .extruded_faces_to_point(
                            apex.translated(shift)?,
                            input.options.solid,
                            tolerance,
                        )?
                        .to_brep(tolerance)?
                        .transformed(
                            AffineTransform3::from_translation(zero.vector_to(origin)?),
                            tolerance,
                        )?;
                    if input.options.solid {
                        Ok(vec![Geometry::Brep(result)])
                    } else {
                        // A singular apex does not join otherwise separate boundary
                        // loops. Rhino emits one object for each edge-connected wall.
                        Ok(result
                            .into_edge_connected_components(tolerance)?
                            .into_iter()
                            .map(Geometry::Brep)
                            .collect())
                    }
                }
                #[cfg(not(feature = "native-smlib"))]
                {
                    let _ = (brep, tolerance, apex);
                    Err(CommandError::SurfaceApexExtrusionRequiresNativeKernel)
                }
            },
        )?;
        Ok(format!("Extruded {count} surface face(s) to the apex"))
    }
}
#[cfg(test)]
mod tests;
