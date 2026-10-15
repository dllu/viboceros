//! Exact surface/face extrusion through the native topology sweep.
use super::*;

pub const USAGE: &str = "ExtrudeSrf distance | base target [BothSides=Yes|No] [Solid=Yes|No] [DeleteInput=Yes|No] [Direction=x,y,z] [Face=index|Faces=All|Faces=i,j] [FaceRefs=id:index,...]";
pub(super) struct ExtrudeSurfaceCommand;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceExtrusionOptions {
    pub both_sides: bool,
    pub solid: bool,
    pub delete_input: bool,
    pub direction: Option<Vector3>,
}
impl Default for SurfaceExtrusionOptions {
    fn default() -> Self {
        Self {
            both_sides: false,
            solid: true,
            delete_input: false,
            direction: None,
        }
    }
}
impl SurfaceExtrusionOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            if !seen.insert(key.to_ascii_lowercase()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.to_ascii_lowercase().as_str() {
                "bothsides" => {
                    next.both_sides = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "solid" => next.solid = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "direction" => {
                    let (p, n) = parse_point(&[value])?;
                    if n != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.direction = Some(
                        Vector3::try_from(p.to_array())?
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
            "BothSides={} Solid={} DeleteInput={}",
            if self.both_sides { "Yes" } else { "No" },
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" }
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
        let mut result = Self::default();
        let mut common = Vec::new();
        let mut face_seen = false;
        for word in words {
            let (key, value) = word
                .trim_start_matches('_')
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
                common.push(*word);
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
        if args.iter().any(|s| !s.contains('=')) {
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
        let (positional, words): (Vec<_>, Vec<_>) =
            args.iter().copied().partition(|s| !s.contains('='));
        let input = SurfaceExtrusionInput::parse(&words)?;
        let distance = if positional.len() == 1 {
            let h = parse_finite_real(positional[0])?;
            if h == 0. {
                return Err(CommandError::Usage(USAGE));
            }
            Distance::Numeric(h)
        } else {
            let (a, n) = parse_point(&positional)?;
            let (b, m) = parse_point(&positional[n..])?;
            if n + m != positional.len() || input.options.direction.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let v = a.vector_to(b)?;
            v.normalized_nonzero()?;
            Distance::Explicit(v)
        };
        let refs = surface_extrusion_targets(doc, &input.faces)?;
        let tolerance = doc.tolerance();
        let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        let mut staged = Vec::new();
        for (id, face) in refs {
            let source = doc.object(id).unwrap().geometry();
            let brep = match source {
                Geometry::Brep(b) => b.duplicate_faces(&[face], tolerance)?,
                Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), tolerance)?,
                _ => unreachable!(),
            };
            staged.push(produce(
                &brep,
                distance,
                input.options,
                context.construction_plane,
                tolerance,
            )?);
            grouped.entry(id).or_default().insert(face);
        }
        let mut remainders = Vec::new();
        if input.options.delete_input {
            for (id, used) in grouped {
                let remainder = match doc.object(id).unwrap().geometry() {
                    Geometry::Brep(b) => {
                        let keep = (0..b.faces().len())
                            .filter(|i| !used.contains(i))
                            .collect::<Vec<_>>();
                        if keep.is_empty() {
                            None
                        } else {
                            Some(Geometry::Brep(b.sub_brep(&keep, tolerance)?))
                        }
                    }
                    Geometry::NurbsSurface(_) => None,
                    _ => unreachable!(),
                };
                remainders.push((id, remainder));
            }
        }
        let mut outputs = Vec::with_capacity(staged.len());
        for geometry in staged {
            outputs.push(doc.add_geometry(geometry)?);
        }
        for (id, remainder) in remainders {
            if let Some(g) = remainder {
                doc.replace_object_geometries([(id, g)])?;
            } else {
                doc.delete_object(id)?;
            }
        }
        let count = outputs.len();
        doc.select_command_results(outputs)?;
        Ok(format!("Extruded {count} surface face(s)"))
    }
}
#[derive(Clone, Copy)]
enum Distance {
    Numeric(f64),
    Explicit(Vector3),
}
fn produce(
    brep: &Brep,
    distance: Distance,
    options: SurfaceExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        use viboceros_smlib::Shape;
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
        let source = Shape::from_brep(&local, tolerance)?;
        let vector = match distance {
            Distance::Explicit(v) => v,
            Distance::Numeric(h) => {
                let normal = if let Some(v) = options.direction {
                    v
                } else if let Some(normal) = source.single_face_plane_normal(tolerance)? {
                    let c = normal.to_array();
                    let i = (0..3)
                        .max_by(|&a, &b| c[a].abs().total_cmp(&c[b].abs()))
                        .unwrap();
                    if c[i] < 0. {
                        normal.scaled(-1.)?
                    } else {
                        normal
                    }
                } else {
                    cplane.z_axis().as_vector()
                };
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
        Ok(Geometry::Brep(result))
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
