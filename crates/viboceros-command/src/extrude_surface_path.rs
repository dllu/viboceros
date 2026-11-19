//! Fixed-orientation surface extrusion along a chosen curve.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, apply_face_extrusions};

pub const USAGE: &str = "ExtrudeSrfAlongCrv [Path=id|PathName=name] [Solid=Yes|No] [DeleteInput=Yes|No] [SubCurve=Yes|No] [Reverse=Auto|Yes|No] [PathLength=length|PathRange=t0,t1] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfacePathCommand {
    pub(super) settings: std::sync::Arc<remembered::Remembered<SurfacePathOptions>>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfacePathOptions {
    pub solid: bool,
    pub delete_input: bool,
    pub subcurve: bool,
    pub reverse: Option<bool>,
    pub path_length: Option<f64>,
    pub path_range: Option<[f64; 2]>,
}
impl SurfacePathOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let word = word.trim_start_matches('_');
            let (key, value) = word
                .split_once('=')
                .map_or((word, None), |(key, value)| (key, Some(value)));
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            let toggle = |current: bool| match value {
                Some(value) => parse_yes_no(value).ok_or(CommandError::Usage(USAGE)),
                None => Ok(!current),
            };
            let require_value = || value.ok_or(CommandError::Usage(USAGE));
            match key.as_str() {
                "solid" => next.solid = toggle(next.solid)?,
                "deleteinput" => next.delete_input = toggle(next.delete_input)?,
                "subcurve" => next.subcurve = toggle(next.subcurve)?,
                "reverse" => {
                    let value = require_value()?;
                    next.reverse = if value.trim_start_matches('_').eq_ignore_ascii_case("Auto") {
                        None
                    } else {
                        Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                    };
                }
                "pathlength" => {
                    let n = parse_finite_real(require_value()?)?;
                    if n <= 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.path_length = Some(n);
                }
                "pathrange" => {
                    let (a, b) = require_value()?
                        .split_once(',')
                        .ok_or(CommandError::Usage(USAGE))?;
                    let a = parse_finite_real(a)?;
                    let b = parse_finite_real(b)?;
                    if a == b {
                        return Err(CommandError::Usage(
                            "PathRange requires two distinct parameters",
                        ));
                    }
                    next.path_range = Some([a.min(b), a.max(b)]);
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        if next.path_length.is_some() && next.path_range.is_some() {
            return Err(CommandError::Usage("Specify PathLength or PathRange"));
        }
        *self = next;
        Ok(())
    }
    fn remembered(self) -> Self {
        Self {
            reverse: None,
            path_length: None,
            path_range: None,
            ..self
        }
    }
    pub fn command_options(self) -> String {
        let mut result = format!(
            "Solid={} DeleteInput={} SubCurve={} Reverse={}",
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" },
            if self.subcurve { "Yes" } else { "No" },
            match self.reverse {
                Some(true) => "Yes",
                Some(false) => "No",
                None => "Auto",
            }
        );
        if let Some(n) = self.path_length {
            result.push_str(&format!(" PathLength={n}"));
        }
        if let Some([a, b]) = self.path_range {
            result.push_str(&format!(" PathRange={a},{b}"));
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
        Self::parse_with_options(words, SurfacePathOptions::default())
    }

    pub fn parse_with_options(
        words: &[&str],
        options: SurfacePathOptions,
    ) -> Result<Self, CommandError> {
        let mut result = Self {
            options,
            ..Self::default()
        };
        let mut face_words = Vec::new();
        let mut option_words = Vec::new();
        for word in words {
            let Some((key, value)) = word.trim_start_matches('_').split_once('=') else {
                option_words.push(*word);
                continue;
            };
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
    if let Some(range) = options.path_range {
        return prepare_surface_path_range(curve, range, options.reverse == Some(true));
    }
    let mut curve = if options.reverse == Some(true) {
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

pub fn prepare_surface_path_range(
    curve: &NurbsCurve,
    [a, b]: [f64; 2],
    reverse: bool,
) -> Result<NurbsCurve, CommandError> {
    let domain = curve.domain();
    let (start, end) = (a.min(b), a.max(b));
    if !a.is_finite()
        || !b.is_finite()
        || start == end
        || !domain.contains(&start)
        || !domain.contains(&end)
    {
        return Err(CommandError::Usage(
            "Pick two distinct points along the path",
        ));
    }
    if !reverse {
        return Ok(curve.try_trimmed(start..=end)?);
    }
    // In reverse SubCurve mode Rhino evaluates the negated original parameters.
    // They can reach beyond either end of the source parameter domain.
    let interval = -end..=-start;
    let extended;
    let source = if domain.contains(interval.start()) && domain.contains(interval.end()) {
        curve
    } else {
        extended = curve.try_extended_to(interval.clone())?;
        &extended
    };
    Ok(source.try_trimmed(interval)?.reversed()?)
}
impl Command for ExtrudeSurfacePathCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrfAlongCrv"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = SurfacePathInput::parse_with_options(args, self.settings.get())?;
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
        let input = SurfacePathInput::parse_with_options(args, self.settings.get())?;
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
        self.settings.set(input.options.remembered());
        Ok(format!("Extruded {count} surface face(s) along the path"))
    }
}
#[cfg(test)]
mod tests;
