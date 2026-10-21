//! Shared staged subdivision for whole objects and explicit face targets.
use crate::*;
use viboceros_document::GeometrySnapshot;

mod arguments;

const USAGE: &str = "SubDivide [Faces=All|0,1,...] [Object=selected-uuid]";

/// A whole target takes precedence over component picks on the same object.
#[derive(Clone, Copy, Debug)]
pub enum SubdivisionTarget {
    Whole(ObjectId),
    Face(ObjectId, usize),
}
#[derive(Clone, Debug)]
struct Replacement {
    id: ObjectId,
    source: GeometrySnapshot,
    geometry: Geometry,
    children: Vec<usize>,
}
/// Complete candidates and immutable sources, prepared outside model history.
#[derive(Clone, Debug)]
pub struct SubdivisionSelection {
    tolerance: Tolerance,
    replacements: Vec<Replacement>,
    meshes: usize,
    subds: usize,
}
#[derive(Clone, Debug)]
pub struct SubdivisionOutcome {
    pub message: String,
    pub selected_faces: Vec<(ObjectId, usize)>,
}

impl SubdivisionSelection {
    pub fn prepare(
        document: &Document,
        targets: impl IntoIterator<Item = SubdivisionTarget>,
    ) -> Result<Self, CommandError> {
        let mut grouped = BTreeMap::<ObjectId, Option<BTreeSet<usize>>>::new();
        let mut order = Vec::new();
        for (n, target) in targets.into_iter().enumerate() {
            if n >= 400_000 {
                return Err(CommandError::Usage(USAGE));
            }
            let (id, face) = match target {
                SubdivisionTarget::Whole(id) => (id, None),
                SubdivisionTarget::Face(id, f) => (id, Some(f)),
            };
            let object = document
                .object(id)
                .filter(|_| document.is_object_selectable(id))
                .ok_or(CommandError::SubdivisionUnavailable)?;
            let count = match object.geometry() {
                Geometry::Mesh(m) => m.face_count(),
                Geometry::SubD(s) => s.faces().len(),
                _ => return Err(CommandError::SubdivisionUnavailable),
            };
            if let Some(face) = face
                && face >= count
            {
                return Err(CommandError::SubdivisionFaceIndexOutOfRange {
                    face,
                    face_count: count,
                });
            }
            let entry = grouped.entry(id).or_insert_with(|| {
                order.push(id);
                Some(BTreeSet::new())
            });
            match (entry, face) {
                (entry, None) => *entry = None,
                (Some(faces), Some(face)) => {
                    faces.insert(face);
                }
                _ => {}
            }
        }
        if order.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut result = Self {
            tolerance: document.tolerance(),
            replacements: Vec::with_capacity(order.len()),
            meshes: 0,
            subds: 0,
        };
        let mut total_vertices = 0_usize;
        let mut total_faces = 0_usize;
        for id in order {
            let object = document.object(id).unwrap();
            let indices = grouped
                .remove(&id)
                .unwrap()
                .map(|s| s.into_iter().collect::<Vec<_>>());
            let mut children = Vec::new();
            let geometry = match object.geometry() {
                Geometry::Mesh(mesh) => {
                    result.meshes += 1;
                    Geometry::Mesh(match &indices {
                        Some(f) => mesh.subdivided_faces(f)?,
                        None => mesh.subdivided()?,
                    })
                }
                Geometry::SubD(subd) => {
                    result.subds += 1;
                    let out = match &indices {
                        Some(f) => {
                            let (out, selected) = subd.subdivided_faces_with_selection(f)?;
                            // An all-face request follows Rhino's whole-object
                            // path, which clears component selection.
                            if f.len() != subd.faces().len() {
                                children = selected;
                            }
                            out
                        }
                        None => subd.subdivided(1)?,
                    };
                    Geometry::SubD(out)
                }
                _ => unreachable!("validated subdivision targets"),
            };
            let (vertices, faces) = match &geometry {
                Geometry::Mesh(m) => (m.vertices().len(), m.face_count()),
                Geometry::SubD(s) => (s.control_points().len(), s.faces().len()),
                _ => unreachable!(),
            };
            total_vertices = total_vertices
                .checked_add(vertices)
                .ok_or(CommandError::SubdivisionResourceLimit)?;
            total_faces = total_faces
                .checked_add(faces)
                .ok_or(CommandError::SubdivisionResourceLimit)?;
            if total_vertices > 4_000_000 || total_faces > 1_000_000 {
                return Err(CommandError::SubdivisionResourceLimit);
            }
            result.replacements.push(Replacement {
                id,
                source: object.geometry_snapshot().clone(),
                geometry,
                children,
            });
        }
        Ok(result)
    }
    pub fn commit(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<SubdivisionOutcome, CommandError> {
        run_command_transaction(document, "SubDivide", |doc| self.apply(doc, postselected))
    }
    fn apply(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<SubdivisionOutcome, CommandError> {
        if document.tolerance() != self.tolerance
            || self.replacements.iter().any(|r| {
                !document.is_object_selectable(r.id)
                    || document
                        .object(r.id)
                        .is_none_or(|o| o.geometry_snapshot() != &r.source)
            })
        {
            return Err(CommandError::SubdivisionStale);
        }
        if postselected {
            document.release_command_selection_on_history_replay(
                self.replacements.iter().map(|r| r.id),
            )?;
            document.clear_selection();
        }
        document.replace_object_geometries(
            self.replacements.iter().map(|r| (r.id, r.geometry.clone())),
        )?;
        // Rhino replaces meshes before SubDs. Stable identity and input
        // attributes remain independent of runtime insertion order.
        document.move_objects_to_end_in_order(
            self.replacements
                .iter()
                .filter(|r| matches!(r.geometry, Geometry::Mesh(_)))
                .chain(
                    self.replacements
                        .iter()
                        .filter(|r| matches!(r.geometry, Geometry::SubD(_))),
                )
                .map(|r| r.id),
        )?;
        Ok(SubdivisionOutcome {
            message: format!(
                "Subdivided {} SubD(s) and {} mesh(es)",
                self.subds, self.meshes
            ),
            selected_faces: self
                .replacements
                .iter()
                .flat_map(|r| r.children.iter().map(move |&f| (r.id, f)))
                .collect(),
        })
    }
}

pub(super) struct SubDivideCommand;
impl Command for SubDivideCommand {
    fn name(&self) -> &'static str {
        "SubDivide"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() {
            arguments::parse(arguments)?;
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::MeshAndSubD,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.divide(document, arguments, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.divide(document, arguments, true)
    }
}
impl SubDivideCommand {
    fn divide(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let targets = arguments::targets(document, arguments)?;
        SubdivisionSelection::prepare(document, targets)?
            .apply(document, postselected)
            .map(|out| out.message)
    }
}
