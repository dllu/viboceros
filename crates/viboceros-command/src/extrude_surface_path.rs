//! Fixed-orientation surface extrusion along a chosen curve.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, apply_face_extrusions};

pub const USAGE: &str = "ExtrudeSrfAlongCrv [Path=id|PathName=name] [Solid=Yes|No] [DeleteInput=Yes|No] [Reverse=Yes|No] [PathLength=length] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfacePathCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePathOptions {
    pub solid: bool,
    pub delete_input: bool,
    pub reverse: bool,
    pub path_length: Option<f64>,
}
impl Default for SurfacePathOptions {
    fn default() -> Self {
        Self {
            solid: true,
            delete_input: false,
            reverse: false,
            path_length: None,
        }
    }
}
impl SurfacePathOptions {
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
                "solid" => next.solid = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "reverse" => {
                    next.reverse = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "pathlength" => {
                    let n = parse_finite_real(value)?;
                    if n <= 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.path_length = Some(n);
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        let mut result = format!(
            "Solid={} DeleteInput={} Reverse={}",
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" },
            if self.reverse { "Yes" } else { "No" }
        );
        if let Some(n) = self.path_length {
            result.push_str(&format!(" PathLength={n}"));
        }
        result
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum SurfacePathSelector {
    Id(ObjectId),
    Name(String),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfacePathInput {
    pub options: SurfacePathOptions,
    pub faces: SurfaceFaces,
    pub path: Option<SurfacePathSelector>,
}
impl SurfacePathInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        let mut result = Self::default();
        let mut face_words = Vec::new();
        let mut option_words = Vec::new();
        for word in words {
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            match key.to_ascii_lowercase().as_str() {
                "path" | "pathname" => {
                    if result.path.is_some() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    result.path = Some(if key.eq_ignore_ascii_case("Path") {
                        SurfacePathSelector::Id(
                            value.parse().map_err(|_| CommandError::Usage(USAGE))?,
                        )
                    } else {
                        if value.is_empty() {
                            return Err(CommandError::Usage(USAGE));
                        }
                        SurfacePathSelector::Name(value.into())
                    });
                }
                "face" | "faces" | "facerefs" => face_words.push(*word),
                _ => option_words.push(*word),
            }
        }
        result.options.update(&option_words)?;
        result.faces = SurfaceExtrusionInput::parse(&face_words)?.faces;
        Ok(result)
    }
}
pub fn resolve_surface_path(
    doc: &Document,
    selector: Option<&SurfacePathSelector>,
) -> Result<ObjectId, CommandError> {
    let ids = match selector {
        Some(SurfacePathSelector::Id(id)) => vec![*id],
        Some(SurfacePathSelector::Name(name)) => doc
            .objects()
            .filter(|o| o.attributes().name() == Some(name.as_str()))
            .map(|o| o.id())
            .collect(),
        None => doc
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect(),
    };
    let [id] = ids.as_slice() else {
        return Err(CommandError::Usage(
            "ExtrudeSrfAlongCrv requires one selected path curve or an explicit Path/PathName",
        ));
    };
    if !doc.is_object_selectable(*id)
        || doc
            .object(*id)
            .is_none_or(|o| o.geometry().curve_ref().is_none())
    {
        return Err(CommandError::Usage(
            "ExtrudeSrfAlongCrv path must be a selectable curve",
        ));
    }
    Ok(*id)
}
pub fn prepare_surface_path(
    curve: &NurbsCurve,
    options: SurfacePathOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    let mut curve = if options.reverse {
        curve.reversed()?
    } else {
        curve.clone()
    };
    if let Some(length) = options.path_length {
        let total = curve.length(tolerance)?;
        if length > total + tolerance.absolute() {
            return Err(CommandError::Usage("PathLength exceeds the path length"));
        }
        if length < total {
            let start = *curve.domain().start();
            let end = curve
                .parameter_at_arc_length_from(start, length, tolerance)?
                .ok_or(CommandError::Usage("PathLength exceeds the path length"))?;
            curve = curve.try_trimmed(start..=end)?;
        }
    }
    Ok(curve)
}
impl Command for ExtrudeSurfacePathCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrfAlongCrv"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = SurfacePathInput::parse(args)?;
        if input.path.is_some() || matches!(input.faces, SurfaceFaces::References(_)) {
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
        let input = SurfacePathInput::parse(args)?;
        let id = resolve_surface_path(doc, input.path.as_ref())?;
        let path = doc
            .object(id)
            .unwrap()
            .geometry()
            .nurbs_curve_representation()?
            .ok_or(CommandError::Usage(USAGE))?;
        let path = prepare_surface_path(&path, input.options, doc.tolerance())?;
        #[cfg(feature = "native-smlib")]
        let native = viboceros_smlib::KernelCurve::from_nurbs_for_modeling(&path)?;
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
                    let result = viboceros_smlib::Shape::from_brep(&local, tolerance)?
                        .extruded_faces_along_curve(&native, input.options.solid, tolerance)?
                        .to_brep(tolerance)?
                        .transformed(
                            AffineTransform3::from_translation(zero.vector_to(origin)?),
                            tolerance,
                        )?;
                    Ok(Geometry::Brep(result))
                }
                #[cfg(not(feature = "native-smlib"))]
                {
                    let _ = (brep, tolerance, &path);
                    Err(CommandError::SurfacePathExtrusionRequiresNativeKernel)
                }
            },
        )?;
        Ok(format!("Extruded {count} surface face(s) along the path"))
    }
}
#[cfg(test)]
mod tests;
