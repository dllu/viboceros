//! Native surface and original face extrusion to an explicit apex.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, apply_face_extrusion_products};
pub use crate::surface_reference::{BoundaryInput, BoundaryRef};
#[cfg(feature = "native-smlib")]
mod boundary;
pub const USAGE: &str = "ExtrudeSrfToPoint apex|0|w0 [ToBoundary=id[:face]] [Solid=Yes|No] [DeleteInput=Yes|No] [SplitAtTangents=Yes|No] [Face=index|Faces=All|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceToPointCommand {
    pub(super) preferences: std::sync::Arc<crate::remembered::Remembered<SurfaceApexOptions>>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceApexOptions {
    pub solid: bool,
    pub delete_input: bool,
    pub split_at_tangents: bool,
}
impl Default for SurfaceApexOptions {
    fn default() -> Self {
        Self {
            solid: false,
            delete_input: false,
            split_at_tangents: false,
        }
    }
}
impl SurfaceApexOptions {
    pub fn recognizes(word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        let key = word.split_once('=').map_or(word, |(key, _)| key);
        ["Solid", "DeleteInput", "SplitAtTangents"]
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
    }

    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let word = word.trim_start_matches(['_', '-']);
            let (key, value) = word
                .split_once('=')
                .map_or((word, None), |(key, value)| (key, Some(value)));
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            let enabled = match key.as_str() {
                "solid" => &mut next.solid,
                "deleteinput" => &mut next.delete_input,
                "splitattangents" => &mut next.split_at_tangents,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            *enabled = match value {
                Some(value) => parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                None => !*enabled,
            };
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Solid={} {}",
            if self.solid { "Yes" } else { "No" },
            self.boundary_options(),
        )
    }

    pub fn boundary_options(self) -> String {
        format!(
            "DeleteInput={} SplitAtTangents={}",
            if self.delete_input { "Yes" } else { "No" },
            if self.split_at_tangents { "Yes" } else { "No" }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApexPoint {
    Explicit(Point3),
    ConstructionPlaneOrigin,
}

impl ApexPoint {
    pub fn origin_token(token: &str) -> Option<Self> {
        let token = token.trim().trim_start_matches('_');
        if token == "0" {
            Some(Self::ConstructionPlaneOrigin)
        } else if token.eq_ignore_ascii_case("w0") {
            Some(Self::Explicit(
                Point3::try_new(0., 0., 0.).expect("finite origin"),
            ))
        } else {
            None
        }
    }

    pub fn resolve(self, construction_plane: Frame3) -> Point3 {
        match self {
            Self::Explicit(point) => point,
            Self::ConstructionPlaneOrigin => construction_plane.origin(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceApexInput {
    pub options: SurfaceApexOptions,
    pub faces: SurfaceFaces,
    pub apex: Option<ApexPoint>,
    pub boundary: BoundaryInput,
}
impl SurfaceApexInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_options(words, SurfaceApexOptions::default())
    }

    pub fn parse_with_options(
        words: &[&str],
        mut defaults: SurfaceApexOptions,
    ) -> Result<Self, CommandError> {
        let (mut faces, mut options, mut positional) = (Vec::new(), Vec::new(), Vec::new());
        let mut boundary = BoundaryInput::None;
        for word in words {
            let token = word.trim_start_matches(['_', '-']);
            if let Some((key, value)) = token.split_once('=') {
                if key.eq_ignore_ascii_case("ToBoundary") {
                    if boundary != BoundaryInput::None {
                        return Err(CommandError::Usage(USAGE));
                    }
                    boundary = BoundaryInput::Reference(BoundaryRef::parse(value)?);
                } else if ["Face", "Faces", "FaceRefs"]
                    .iter()
                    .any(|s| key.eq_ignore_ascii_case(s))
                {
                    faces.push(*word);
                } else {
                    options.push(*word);
                }
            } else if token.eq_ignore_ascii_case("ToBoundary") {
                if boundary != BoundaryInput::None {
                    return Err(CommandError::Usage(USAGE));
                }
                boundary = BoundaryInput::Select;
            } else if SurfaceApexOptions::recognizes(word) {
                options.push(*word);
            } else {
                positional.push(*word);
            }
        }
        defaults.update(&options)?;
        let mut result = Self {
            faces: SurfaceExtrusionInput::parse(&faces)?.faces,
            options: defaults,
            boundary,
            ..Self::default()
        };
        if !positional.is_empty() {
            result.apex = if let [token] = positional.as_slice()
                && let Some(origin) = ApexPoint::origin_token(token)
            {
                Some(origin)
            } else {
                let (apex, count) = parse_point(&positional)?;
                require_consumed(&positional, count, USAGE)?;
                Some(ApexPoint::Explicit(apex))
            };
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
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let input = SurfaceApexInput::parse_with_options(args, self.preferences.get())?;
        let apex = input
            .apex
            .ok_or(CommandError::Usage(USAGE))?
            .resolve(context.construction_plane);
        let boundary = match input.boundary {
            BoundaryInput::None => None,
            BoundaryInput::Reference(reference) => Some(reference.capture(doc)?),
            BoundaryInput::Select => {
                return Err(CommandError::Usage("ToBoundary requires a surface"));
            }
        };
        let capped = input.options.solid || boundary.is_some();
        let count = apply_face_extrusion_products(
            doc,
            &input.faces,
            input.options.delete_input,
            |brep, tolerance| {
                #[cfg(feature = "native-smlib")]
                {
                    let coplanar = if boundary.is_none() {
                        brep.try_extruded_face_to_coplanar_point(
                            apex,
                            capped,
                            input.options.split_at_tangents,
                            tolerance,
                        )?
                    } else {
                        None
                    };
                    let result = match coplanar {
                        Some(viboceros_geometry::CoplanarFaceExtrusion::Brep(brep)) => brep,
                        Some(viboceros_geometry::CoplanarFaceExtrusion::ApexOnly) => {
                            return Ok(vec![Geometry::Point(apex)]);
                        }
                        None => native_extrusion(
                            &brep,
                            apex,
                            boundary.as_ref(),
                            input.options,
                            tolerance,
                        )?,
                    };
                    if capped {
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
                    let _ = (brep, tolerance, apex, capped);
                    Err(CommandError::SurfaceApexExtrusionRequiresNativeKernel)
                }
            },
        )?;
        self.preferences.set(input.options);
        let target = if boundary.is_some() {
            "boundary"
        } else {
            "apex"
        };
        Ok(format!("Extruded {count} surface face(s) to the {target}"))
    }
}

#[cfg(feature = "native-smlib")]
fn native_extrusion(
    brep: &Brep,
    apex: Point3,
    boundary: Option<&Brep>,
    options: SurfaceApexOptions,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let zero = Point3::try_new(0., 0., 0.)?;
    let origin = brep
        .vertices()
        .first()
        .map(|v| v.point())
        .unwrap_or_else(|| brep.bounds().min());
    let shift = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let local = brep.transformed(shift, tolerance)?;
    let apex = shift.transform_point(apex)?;
    let boundary = boundary
        .map(|face| face.transformed(shift, tolerance))
        .transpose()?;
    let result = if let Some(boundary) = &boundary {
        boundary::extrude(&local, boundary, apex, options.split_at_tangents, tolerance)?
    } else {
        viboceros_smlib::Shape::from_brep(&local, tolerance)?
            .extruded_faces_to_point_with_tangents(
                apex,
                options.solid,
                options.split_at_tangents,
                tolerance,
            )?
            .to_brep(tolerance)?
    };
    Ok(result.transformed(
        AffineTransform3::from_translation(zero.vector_to(origin)?),
        tolerance,
    )?)
}
#[cfg(test)]
mod tests;
