//! Immutable conversion candidates and the shared ToSubD command policy.
use crate::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{SubD, SubDFromMeshOptions};
mod options;
pub use options::ToSubDOptions;
const USAGE: &str = "ToSubD [UseMesh=ControlPoints|Location] [MeshCreases=Yes|No] [MeshCorners=Yes|No] [UseSurface=ControlPoints|Location] [SurfaceCorners=Yes|No] [DeleteInput=Yes|No] [Faces=All|0,1,...] [Object=selected-uuid]";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToSubDTarget {
    Whole(ObjectId),
    Face(ObjectId, usize),
}
#[derive(Clone, Debug)]
struct Candidate {
    id: ObjectId,
    geometry: Geometry,
}
#[derive(Clone, Debug)]
pub struct ToSubDSelection {
    tolerance: Tolerance,
    options: ToSubDOptions,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    candidates: Vec<Candidate>,
}
impl ToSubDSelection {
    pub fn prepare(
        document: &Document,
        targets: impl IntoIterator<Item = ToSubDTarget>,
        options: ToSubDOptions,
    ) -> Result<Self, CommandError> {
        let mut grouped = BTreeMap::<ObjectId, Option<Vec<usize>>>::new();
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, target) in targets.into_iter().enumerate() {
            if i >= 100_000 {
                return Err(CommandError::ToSubDResourceLimit);
            }
            if !seen.insert(target) {
                continue;
            }
            let (id, face) = match target {
                ToSubDTarget::Whole(id) => (id, None),
                ToSubDTarget::Face(id, f) => (id, Some(f)),
            };
            let object = document
                .object(id)
                .filter(|_| document.is_object_selectable(id))
                .ok_or(CommandError::ToSubDUnavailable)?;
            if let Some(f) = face {
                let count = match object.geometry() {
                    Geometry::NurbsSurface(_) => 1,
                    Geometry::Brep(b) => b.faces().len(),
                    _ => return Err(CommandError::ToSubDUnavailable),
                };
                if f >= count {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if !accepts(object.geometry()) {
                return Err(CommandError::ToSubDUnavailable);
            }
            let entry = grouped.entry(id).or_insert_with(|| {
                order.push(id);
                Some(Vec::new())
            });
            match (entry, face) {
                (entry, None) => *entry = None,
                (Some(faces), Some(f)) => faces.push(f),
                _ => {}
            }
        }
        if order.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut result = Self {
            tolerance: document.tolerance(),
            options,
            sources: Vec::new(),
            candidates: Vec::new(),
        };
        let (mut vertices, mut faces) = (0_usize, 0_usize);
        for id in order {
            let object = document.object(id).unwrap();
            result
                .sources
                .push((id, object.geometry_snapshot().clone()));
            let selected = grouped.remove(&id).unwrap();
            // Borrow input geometry; only converted candidates consume the aggregate budget.
            let inputs: Vec<_> = match (&selected, object.geometry()) {
                (Some(indices), Geometry::Brep(b)) => indices
                    .iter()
                    .map(|&i| ConversionInput::Surface(b.faces()[i].surface()))
                    .collect(),
                (_, Geometry::Mesh(m)) => vec![ConversionInput::Mesh(m)],
                (_, Geometry::NurbsSurface(s)) => vec![ConversionInput::Surface(s)],
                (_, Geometry::Brep(b)) if b.is_extrusion() => {
                    vec![ConversionInput::Extrusion(b.extrusion().unwrap())]
                }
                (_, Geometry::Brep(b)) if b.faces().len() == 1 => {
                    vec![ConversionInput::Surface(b.faces()[0].surface())]
                }
                _ => return Err(CommandError::ToSubDUnavailable),
            };
            let candidate_start = result.candidates.len();
            for input in inputs {
                let converted = match input {
                    ConversionInput::Mesh(m) => SubD::try_from_mesh(
                        m,
                        SubDFromMeshOptions {
                            interpolate: options.mesh_location,
                            corners: options.mesh_corners,
                            creases: options.mesh_creases,
                        },
                        document.tolerance(),
                    ),
                    ConversionInput::Surface(s) => SubD::try_from_surface(
                        s,
                        options.surface_method,
                        options.surface_corners,
                        document.tolerance(),
                    ),
                    ConversionInput::Extrusion(e) => SubD::try_from_extrusion(
                        e,
                        options.surface_method,
                        options.surface_corners,
                        document.tolerance(),
                    ),
                };
                let subd = match converted {
                    Ok(s) => s,
                    Err(GeometryError::EmptyMesh) => continue,
                    Err(e) => return Err(e.into()),
                };
                vertices = vertices.saturating_add(subd.control_points().len());
                faces = faces.saturating_add(subd.faces().len());
                if vertices > 4_000_000 || faces > 1_000_000 {
                    return Err(CommandError::ToSubDResourceLimit);
                }
                result.candidates.push(Candidate {
                    id,
                    geometry: Geometry::SubD(subd),
                });
            }
            if selected.is_some() && !options.delete_input {
                // Native copies are appended in reverse component order. Replacements
                // instead visit forward, retaining the last selected face's result.
                result.candidates[candidate_start..].reverse();
            }
        }
        Ok(result)
    }
    pub fn commit(&self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "ToSubD", |doc| self.apply(doc))
    }
    fn apply(&self, document: &mut Document) -> Result<String, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, g)| {
                !document.is_object_selectable(*id)
                    || document
                        .object(*id)
                        .is_none_or(|o| o.geometry_snapshot() != g)
            })
        {
            return Err(CommandError::ToSubDStale);
        }
        if !self.candidates.is_empty() {
            document.release_command_selection_on_history_replay(
                self.sources.iter().map(|(id, _)| *id),
            )?;
            if self.options.delete_input {
                document.replace_object_geometries(
                    self.candidates.iter().map(|c| (c.id, c.geometry.clone())),
                )?;
                let mut seen = BTreeSet::new();
                let ids = self
                    .candidates
                    .iter()
                    .filter_map(|c| seen.insert(c.id).then_some(c.id))
                    .collect::<Vec<_>>();
                document.move_objects_to_end_in_order(ids.iter().copied())?;
                document.select_objects_direct(ids, SelectionMode::Remove)?;
            } else {
                for c in &self.candidates {
                    let attributes = document.object(c.id).unwrap().attributes().clone();
                    document.add_geometry_with_attributes(c.geometry.clone(), attributes)?;
                }
            }
        }
        Ok(format!("Created {} SubD(s)", self.candidates.len()))
    }
}
enum ConversionInput<'a> {
    Mesh(&'a TriangleMesh),
    Surface(&'a NurbsSurface),
    Extrusion(&'a viboceros_geometry::ExtrusionRepresentation),
}
pub fn accepts(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Mesh(_) | Geometry::NurbsSurface(_))
        || matches!(geometry,Geometry::Brep(b)if b.is_extrusion()||b.faces().len()==1)
}
#[derive(Default)]
pub(super) struct ToSubDCommand {
    options: remembered::Remembered<ToSubDOptions>,
}
impl Command for ToSubDCommand {
    fn name(&self) -> &'static str {
        "ToSubD"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let parsed = options::parse(args, self.options.get())?;
        if parsed.faces.is_some() {
            return Ok(None);
        }
        Ok(Some(options::prompt(parsed.options)))
    }
    fn object_selection_confirmation(
        &self,
        _doc: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(options::prompt(
            options::parse(args, self.options.get())?.options,
        )))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.options
            .set(options::parse(args, self.options.get())?.options);
        Ok(())
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let parsed = options::parse(args, self.options.get())?;
        let mut targets = Vec::new();
        if parsed.object.is_some_and(|id| !document.is_selected(id)) {
            return Err(CommandError::Usage(USAGE));
        }
        for object in document
            .selected_objects()
            .filter(|o| parsed.object.is_none_or(|id| o.id() == id))
        {
            match &parsed.faces {
                None if accepts(object.geometry()) => {
                    targets.push(ToSubDTarget::Whole(object.id()))
                }
                Some(request) => {
                    let count = match object.geometry() {
                        Geometry::Brep(b) => b.faces().len(),
                        Geometry::NurbsSurface(_) => 1,
                        _ => continue,
                    };
                    let indices = match request {
                        SurfaceFaceIndices::All => (0..count).collect(),
                        SurfaceFaceIndices::Indices(indices) => indices.clone(),
                    };
                    targets.extend(
                        indices
                            .into_iter()
                            .map(|f| ToSubDTarget::Face(object.id(), f)),
                    );
                }
                _ => {}
            }
        }
        let selection = ToSubDSelection::prepare(document, targets, parsed.options)?;
        if parsed.faces.is_some() {
            document.select_objects_direct(
                selection
                    .sources
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>(),
                SelectionMode::Remove,
            )?;
        }
        self.options.set(parsed.options);
        selection.apply(document)
    }
}
