//! Exact surface/face extrusion through the native topology sweep.
use super::*;
mod apply;
pub(super) use apply::{apply_face_extrusion_products, apply_face_extrusions};

pub const USAGE: &str = "ExtrudeSrf distance | base target [BothSides=Yes|No] [Solid=Yes|No] [DeleteInput=Yes|No] [SplitAtTangents=Yes|No] [Direction=x,y,z] [Face=index|Faces=All|Faces=i,j] [FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceCommand {
    pub(super) settings: std::sync::Arc<remembered::Remembered<SurfaceExtrusionSettings>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceExtrusionSettings {
    pub options: SurfaceExtrusionOptions,
    pub distance: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceExtrusionOptions {
    pub both_sides: bool,
    pub solid: bool,
    pub delete_input: bool,
    pub split_at_tangents: bool,
    pub direction: Option<Vector3>,
}
impl Default for SurfaceExtrusionOptions {
    fn default() -> Self {
        Self {
            both_sides: false,
            solid: true,
            delete_input: false,
            split_at_tangents: false,
            direction: None,
        }
    }
}
impl SurfaceExtrusionOptions {
    pub fn recognizes(word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(key, value)| (key, Some(value)));
        ["BothSides", "Solid", "DeleteInput", "SplitAtTangents"]
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
            || (value.is_some() && key.eq_ignore_ascii_case("Direction"))
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
            let flag = match key.as_str() {
                "bothsides" => &mut next.both_sides,
                "solid" => &mut next.solid,
                "deleteinput" => &mut next.delete_input,
                "splitattangents" => &mut next.split_at_tangents,
                "direction" => {
                    let value = value.ok_or(CommandError::Usage(USAGE))?;
                    let (p, n) = parse_point(&[value])?;
                    if n != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.direction = Some(
                        Vector3::try_from(p.to_array())?
                            .normalized_nonzero()?
                            .as_vector(),
                    );
                    continue;
                }
                _ => return Err(CommandError::Usage(USAGE)),
            };
            *flag = value
                .map_or(Some(!*flag), parse_yes_no)
                .ok_or(CommandError::Usage(USAGE))?;
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        let mut result = format!(
            "BothSides={} Solid={} DeleteInput={} SplitAtTangents={}",
            if self.both_sides { "Yes" } else { "No" },
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" },
            if self.split_at_tangents { "Yes" } else { "No" },
        );
        if let Some(v) = self.direction {
            let [x, y, z] = v.to_array();
            result.push_str(&format!(" Direction={x},{y},{z}"));
        }
        result
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SurfaceFaces {
    #[default]
    Single,
    All,
    Indices(Vec<usize>),
    References(Vec<(ObjectId, usize)>),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceExtrusionInput {
    pub options: SurfaceExtrusionOptions,
    pub faces: SurfaceFaces,
}
impl SurfaceExtrusionInput {
    /// Parse an option-only command start, including explicit face targets.
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_options(words, SurfaceExtrusionOptions::default())
    }

    pub fn parse_with_options(
        words: &[&str],
        options: SurfaceExtrusionOptions,
    ) -> Result<Self, CommandError> {
        let mut result = Self {
            options,
            faces: SurfaceFaces::Single,
        };
        let mut common = Vec::new();
        let mut face_seen = false;
        for word in words {
            if SurfaceExtrusionOptions::recognizes(word) {
                common.push(*word);
                continue;
            }
            let (key, value) = word
                .trim_start_matches(['_', '-'])
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            if matches!(
                key.to_ascii_lowercase().as_str(),
                "face" | "faces" | "facerefs"
            ) {
                if face_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                face_seen = true;
                result.faces = if key.eq_ignore_ascii_case("FaceRefs") {
                    let refs = value
                        .split(',')
                        .map(|s| {
                            let (id, face) =
                                s.rsplit_once(':').ok_or(CommandError::Usage(USAGE))?;
                            Ok((
                                id.parse::<ObjectId>()
                                    .map_err(|_| CommandError::Usage(USAGE))?,
                                face.parse::<usize>()
                                    .map_err(|_| CommandError::Usage(USAGE))?,
                            ))
                        })
                        .collect::<Result<Vec<_>, CommandError>>()?;
                    SurfaceFaces::References(refs)
                } else if key.eq_ignore_ascii_case("Faces") && value.eq_ignore_ascii_case("All") {
                    SurfaceFaces::All
                } else {
                    let indices = value
                        .split(',')
                        .map(|n| n.parse::<usize>().map_err(|_| CommandError::Usage(USAGE)))
                        .collect::<Result<Vec<_>, _>>()?;
                    if key.eq_ignore_ascii_case("Face") && indices.len() != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    SurfaceFaces::Indices(indices)
                };
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        result.options.update(&common)?;
        Ok(result)
    }
}
/// Resolve selectable object identities and original face indices without mutation.
pub fn surface_extrusion_targets(
    doc: &Document,
    faces: &SurfaceFaces,
) -> Result<Vec<(ObjectId, usize)>, CommandError> {
    let mut refs = if let SurfaceFaces::References(refs) = faces {
        refs.clone()
    } else {
        let mut refs = Vec::new();
        for object in doc
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
        {
            let count = match object.geometry() {
                Geometry::NurbsSurface(_) => 1,
                Geometry::Brep(b) => b.faces().len(),
                _ => unreachable!(),
            };
            match faces {
                SurfaceFaces::Single if count == 1 => refs.push((object.id(), 0)),
                SurfaceFaces::Single => {
                    return Err(CommandError::Usage(
                        "ExtrudeSrf: specify Face=index or Faces=All for a polysurface",
                    ));
                }
                SurfaceFaces::All => refs.extend((0..count).map(|i| (object.id(), i))),
                SurfaceFaces::Indices(indices) => {
                    refs.extend(indices.iter().map(|i| (object.id(), *i)))
                }
                SurfaceFaces::References(_) => unreachable!(),
            }
        }
        refs
    };
    let mut seen = BTreeSet::new();
    refs.retain(|r| seen.insert(*r));
    if refs.is_empty() {
        return Err(CommandError::NoExtractableSurfaces);
    }
    if refs.len() > 100000 {
        return Err(CommandError::Usage(
            "ExtrudeSrf exceeds 100000 source faces",
        ));
    }
    for (id, index) in &refs {
        let object = doc
            .object(*id)
            .filter(|_| doc.is_object_selectable(*id))
            .ok_or(CommandError::NoExtractableSurfaces)?;
        let count = match object.geometry() {
            Geometry::NurbsSurface(_) => 1,
            Geometry::Brep(b) => b.faces().len(),
            _ => return Err(CommandError::UnsupportedExtractSurfaceGeometry),
        };
        if *index >= count {
            return Err(CommandError::Usage("ExtrudeSrf face index is out of range"));
        }
    }
    Ok(refs)
}
impl Command for ExtrudeSurfaceCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSrf"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args
            .iter()
            .any(|s| !s.contains('=') && !SurfaceExtrusionOptions::recognizes(s))
        {
            return Ok(None);
        }
        let input = SurfaceExtrusionInput::parse(args)?;
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
        let (positional, words): (Vec<_>, Vec<_>) = args
            .iter()
            .copied()
            .partition(|s| !s.contains('=') && !SurfaceExtrusionOptions::recognizes(s));
        let input = SurfaceExtrusionInput::parse_with_options(&words, self.settings.get().options)?;
        let distance =
            parse_face_extrusion_distance(&positional, input.options.direction.is_some(), USAGE)?;
        // Whole-object preselection stays on retained inputs. Explicit face
        // targeting keeps the existing component-result selection policy.
        let retained_selection = (!input.options.delete_input
            && matches!(input.faces, SurfaceFaces::Single))
        .then(|| doc.selected_object_ids().collect::<Vec<_>>());
        let display_distance = match distance {
            Distance::Numeric(height) => height,
            Distance::Explicit(vector) => vector.length()?,
        };
        let count = apply_face_extrusion_products(
            doc,
            &input.faces,
            input.options.delete_input,
            |brep, tolerance| {
                produce(
                    brep,
                    distance,
                    input.options,
                    context.construction_plane,
                    tolerance,
                )
            },
        )?;
        if let Some(selection) = retained_selection {
            doc.select_command_results(selection)?;
        }
        self.settings.set(SurfaceExtrusionSettings {
            options: SurfaceExtrusionOptions {
                direction: None,
                ..input.options
            },
            distance: Some(display_distance),
        });
        Ok(format!("Extruded {count} surface face(s)"))
    }
}
#[derive(Clone, Copy)]
pub(super) enum Distance {
    Numeric(f64),
    Explicit(Vector3),
}
pub(super) fn parse_face_extrusion_distance(
    positional: &[&str],
    has_direction: bool,
    usage: &'static str,
) -> Result<Distance, CommandError> {
    if positional.len() == 1 {
        let h = parse_finite_real(positional[0])?;
        if h == 0. {
            return Err(CommandError::Usage(usage));
        }
        Ok(Distance::Numeric(h))
    } else {
        let (a, n) = parse_point(positional)?;
        let (b, m) = parse_point(&positional[n..])?;
        if n + m != positional.len() || has_direction {
            return Err(CommandError::Usage(usage));
        }
        let v = a.vector_to(b)?;
        v.normalized_nonzero()?;
        Ok(Distance::Explicit(v))
    }
}
#[cfg(feature = "native-smlib")]
pub(super) fn face_extrusion_direction(
    source: &viboceros_smlib::Shape,
    direction: Option<Vector3>,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Vector3, CommandError> {
    if let Some(v) = direction {
        return Ok(v);
    }
    Ok(source
        .single_face_plane_normal(tolerance)?
        .map(oriented_plane_normal)
        .transpose()?
        .unwrap_or(cplane.z_axis().as_vector()))
}

fn oriented_plane_normal(normal: Vector3) -> Result<Vector3, GeometryError> {
    let c = normal.to_array();
    let i = (0..3)
        .max_by(|&a, &b| c[a].abs().total_cmp(&c[b].abs()))
        .unwrap();
    if c[i] < 0. {
        normal.scaled(-1.)
    } else {
        Ok(normal)
    }
}

/// Cached by the distance getter to match the committed numeric extrusion.
/// Curved faces leave the direction to the construction plane or explicit axis.
pub fn surface_extrusion_reference_normal(
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<Option<Vector3>, CommandError> {
    #[cfg(feature = "native-smlib")]
    let normal = {
        let (_, local) = local_sweep_source(brep, tolerance)?;
        let source = viboceros_smlib::Shape::from_brep(&local, tolerance)?;
        source.single_face_plane_normal(tolerance)?
    };
    #[cfg(not(feature = "native-smlib"))]
    let normal = match brep.faces() {
        [face] => face
            .surface()
            .plane(tolerance)?
            .map(|plane| plane.normal().as_vector()),
        _ => None,
    };
    Ok(normal.map(oriented_plane_normal).transpose()?)
}

#[cfg(feature = "native-smlib")]
fn local_sweep_source(brep: &Brep, tolerance: Tolerance) -> Result<(Point3, Brep), CommandError> {
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
    Ok((origin, local))
}
fn produce(
    brep: &Brep,
    distance: Distance,
    options: SurfaceExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let (origin, mut local) = local_sweep_source(brep, tolerance)?;
        let splits = local.sweep_boundary_splits(options.split_at_tangents)?;
        if !splits.is_empty() {
            local = local.try_split_edges_at_parameters(&splits, tolerance)?;
        }
        let source = viboceros_smlib::Shape::from_brep(&local, tolerance)?;
        let zero = Point3::try_new(0., 0., 0.)?;
        let vector = match distance {
            Distance::Explicit(v) => v,
            Distance::Numeric(h) => {
                let normal =
                    face_extrusion_direction(&source, options.direction, cplane, tolerance)?;
                normal.scaled(h)?
            }
        };
        let (start, displacement) = if options.both_sides {
            (vector.scaled(-1.)?, vector.scaled(2.)?)
        } else {
            (Vector3::try_new(0., 0., 0.)?, vector)
        };
        let result = source
            .extruded_faces(displacement, options.solid, tolerance)?
            .to_brep(tolerance)?
            .transformed(
                AffineTransform3::from_translation(zero.vector_to(origin.translated(start)?)?),
                tolerance,
            )?;
        if options.solid && !result.is_solid() {
            return Err(CommandError::Usage(
                "ExtrudeSrf did not produce a closed solid",
            ));
        }
        if options.solid {
            Ok(vec![Geometry::Brep(result)])
        } else {
            Ok(result
                .into_edge_connected_components(tolerance)?
                .into_iter()
                .map(Geometry::Brep)
                .collect())
        }
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        match distance {
            Distance::Numeric(h) => {
                let _ = h;
            }
            Distance::Explicit(v) => {
                let _ = v;
            }
        }
        let _ = (brep, options, cplane, tolerance);
        Err(CommandError::SurfaceExtrusionRequiresNativeKernel)
    }
}
#[cfg(test)]
mod tests;
