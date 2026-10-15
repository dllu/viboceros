//! Hollow selected solid faces through the native shell/Boolean executive.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, surface_extrusion_targets};
pub const USAGE: &str =
    "Shell [Thickness=distance] [DeleteInput=Yes|No] [Face=index|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ShellCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellOptions {
    pub thickness: f64,
    pub delete_input: bool,
}
impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            thickness: 1.,
            delete_input: true,
        }
    }
}
impl ShellOptions {
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
            match key.as_str() {
                "thickness" => {
                    let n = parse_finite_real(value)?;
                    if n <= 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.thickness = n;
                }
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Thickness={} DeleteInput={}",
            self.thickness,
            if self.delete_input { "Yes" } else { "No" }
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellInput {
    pub options: ShellOptions,
    pub faces: Option<SurfaceFaces>,
}
impl ShellInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        let (mut faces, mut options) = (Vec::new(), Vec::new());
        for word in words {
            if word
                .trim_start_matches('_')
                .split_once('=')
                .is_some_and(|(k, _)| {
                    ["Face", "Faces", "FaceRefs"]
                        .iter()
                        .any(|s| k.eq_ignore_ascii_case(s))
                })
            {
                faces.push(*word);
            } else {
                options.push(*word);
            }
        }
        let mut result = Self::default();
        result.options.update(&options)?;
        if !faces.is_empty() {
            result.faces = Some(
                SurfaceExtrusionInput::parse(&faces)
                    .map_err(|_| CommandError::Usage(USAGE))?
                    .faces,
            );
        }
        Ok(result)
    }
}
pub fn shell_targets(
    doc: &Document,
    faces: &SurfaceFaces,
) -> Result<BTreeMap<ObjectId, Vec<usize>>, CommandError> {
    let refs = surface_extrusion_targets(doc, faces).map_err(|_| CommandError::Usage(USAGE))?;
    let mut result = BTreeMap::<ObjectId, Vec<usize>>::new();
    for (id, index) in refs {
        let Some(Geometry::Brep(b)) = doc.object(id).map(|o| o.geometry()) else {
            return Err(CommandError::Usage(
                "Shell requires closed manifold polysurfaces",
            ));
        };
        if !b.is_solid() || b.faces().len() < 2 {
            return Err(CommandError::Usage(
                "Shell requires closed manifold polysurfaces",
            ));
        }
        result.entry(id).or_default().push(index);
    }
    for (id, indices) in &result {
        let Geometry::Brep(b) = doc.object(*id).unwrap().geometry() else {
            unreachable!()
        };
        if indices.len() >= b.faces().len() {
            return Err(CommandError::Usage(
                "Shell must retain at least one face on every source",
            ));
        }
    }
    Ok(result)
}
impl Command for ShellCommand {
    fn name(&self) -> &'static str {
        "Shell"
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        let input = ShellInput::parse(args)?;
        Ok(input.faces.is_none().then_some(ComponentSelectionPrompt {
            command: "Shell",
            kind: ComponentSelectionKind::BrepFace,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: input.options.delete_input,
                aliases: &[],
            }],
            numbers: vec![NumberSelectionOption {
                name: "Thickness",
                value: input.options.thickness,
                minimum: 0.,
            }],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = ShellInput::parse(args)?;
        let faces = input.faces.ok_or(CommandError::Usage(
            "Shell requires faces to remove; specify Face/Faces/FaceRefs",
        ))?;
        let sources = shell_targets(doc, &faces)?;
        let staged = stage_shell(doc, &sources, input.options.thickness)?;
        let outputs = doc.copy_object_pieces_into_source_groups(staged)?;
        if input.options.delete_input {
            doc.delete_objects(sources.keys().copied())?;
        }
        doc.select_command_results(outputs.iter().copied())?;
        Ok(format!("Shelled {} solid object(s)", outputs.len()))
    }
}
#[cfg(not(feature = "native-smlib"))]
fn stage_shell(
    _doc: &Document,
    _sources: &BTreeMap<ObjectId, Vec<usize>>,
    _thickness: f64,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    Err(CommandError::ShellRequiresNativeKernel)
}
#[cfg(feature = "native-smlib")]
fn stage_shell(
    doc: &Document,
    sources: &BTreeMap<ObjectId, Vec<usize>>,
    thickness: f64,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let mut staged = Vec::new();
    let tolerance = doc.tolerance();
    for (id, indices) in sources {
        let Geometry::Brep(b) = doc.object(*id).unwrap().geometry() else {
            unreachable!()
        };
        let origin = b
            .vertices()
            .first()
            .map(|v| v.point())
            .unwrap_or_else(|| b.bounds().min());
        let zero = Point3::try_new(0., 0., 0.)?;
        let local = b.transformed(
            AffineTransform3::from_translation(origin.vector_to(zero)?),
            tolerance,
        )?;
        let result = viboceros_smlib::Shape::from_brep(&local, tolerance)?
            .shelled_faces(indices, thickness)?
            .to_brep_approximated(tolerance, tolerance.absolute().min(1e-8))?
            .transformed(
                AffineTransform3::from_translation(zero.vector_to(origin)?),
                tolerance,
            )?;
        if !result.is_solid() {
            return Err(CommandError::Usage(
                "Shell did not produce a closed wall volume",
            ));
        }
        let components = result.edge_connected_face_components();
        if components.len() == 1 {
            staged.push((*id, Geometry::Brep(result)));
        } else {
            // Keep inward cavity boundaries with their enclosing material.
            let (_, regions) = viboceros_smlib::Solid::from_brep_with_regions(&result, tolerance)?;
            for region in regions {
                let faces = region
                    .iter()
                    .flat_map(|&i| components[i].iter().copied())
                    .collect::<Vec<_>>();
                staged.push((*id, Geometry::Brep(result.sub_brep(&faces, tolerance)?)));
            }
        }
    }
    Ok(staged)
}
#[cfg(test)]
mod tests;
