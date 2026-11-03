//! Transactional face insets on editable SubDs and stored mesh polygons.
use crate::*;
use viboceros_document::{GeometrySnapshot, GroupId};
mod arguments;
#[cfg(test)]
mod tests;
pub use arguments::{Input, Mode, Options, parse};
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    geometry: GeometrySnapshot,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub object: ObjectId,
    pub index: usize,
    pub kind: ComponentSelectionKind,
}
#[derive(Clone, Debug)]
pub struct Plan {
    sources: Vec<Source>,
    outputs: Vec<(ObjectId, Geometry)>,
    faces: Vec<Face>,
    selected: Vec<ObjectId>,
    tolerance: Tolerance,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        targets: impl IntoIterator<Item = (ObjectId, usize)>,
        options: Options,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (i, (id, face)) in targets.into_iter().enumerate() {
            if i >= 1_000_000 {
                return Err(too_many_span_outputs("Inset"));
            }
            grouped.entry(id).or_default().insert(face);
        }
        if grouped.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sources = vec![];
        let mut outputs = vec![];
        let mut faces = vec![];
        let mut bytes = 0usize;
        for (id, indices) in grouped {
            let object = doc
                .object(id)
                .filter(|_| doc.is_object_selectable(id))
                .ok_or(CommandError::InsetStale)?;
            let indices = indices.into_iter().collect::<Vec<_>>();
            let (geometry, kind, size) = match object.geometry() {
                Geometry::SubD(s) => {
                    let part =
                        s.try_inset_faces(&indices, options.distance, options.mode == Mode::Group)?;
                    let bytes = part.archive().len();
                    (
                        Geometry::SubD(part),
                        ComponentSelectionKind::SubDFace,
                        bytes,
                    )
                }
                Geometry::Mesh(m) => {
                    let part = m.try_inset_faces(&indices, options.distance)?;
                    let bytes = part
                        .vertices()
                        .len()
                        .saturating_mul(64)
                        .saturating_add(part.face_count().saturating_mul(16));
                    (
                        Geometry::Mesh(part),
                        ComponentSelectionKind::MeshFace,
                        bytes,
                    )
                }
                _ => return Err(CommandError::UnsupportedInsetGeometry),
            };
            bytes = bytes.saturating_add(size);
            if bytes > 64 * 1024 * 1024 || outputs.len() >= MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("Inset"));
            }
            faces.extend(indices.into_iter().map(|index| Face {
                object: id,
                index,
                kind,
            }));
            outputs.push((id, geometry));
            sources.push(Source {
                id,
                geometry: object.geometry_snapshot().clone(),
                attributes: object.attributes().clone(),
                groups: object.group_ids().to_vec(),
            });
        }
        Ok(Self {
            sources,
            outputs,
            faces,
            selected: doc.selected_objects().map(|o| o.id()).collect(),
            tolerance: doc.tolerance(),
        })
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc
                .selected_objects()
                .map(|o| o.id())
                .eq(self.selected.iter().copied())
            && self.sources.iter().all(|s| {
                doc.is_object_selectable(s.id)
                    && doc.object(s.id).is_some_and(|o| {
                        o.geometry_snapshot() == &s.geometry
                            && o.attributes() == &s.attributes
                            && o.group_ids() == s.groups
                    })
            })
    }
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }
    pub fn preview_edges(&self) -> Result<Vec<LineSegment>, CommandError> {
        let mut lines = vec![];
        for (_, geometry) in &self.outputs {
            let pairs = match geometry {
                Geometry::SubD(s) => {
                    if s.edges().len().saturating_add(lines.len()) > 400_000 {
                        return Err(too_many_span_outputs("Inset"));
                    }
                    s.edges()
                        .iter()
                        .map(|e| {
                            [
                                s.control_points()[e[0] as usize],
                                s.control_points()[e[1] as usize],
                            ]
                        })
                        .collect::<Vec<_>>()
                }
                Geometry::Mesh(m) => {
                    if m.face_count().saturating_mul(4).saturating_add(lines.len()) > 400_000 {
                        return Err(too_many_span_outputs("Inset"));
                    }
                    m.topology_edge_points()
                }
                _ => unreachable!(),
            };
            lines.extend(pairs.into_iter().filter_map(|[a, b]| {
                LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION).ok()
            }));
        }
        Ok(lines)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, "Inset", |doc| self.apply(doc))
    }
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::InsetStale);
        }
        doc.replace_object_geometries(self.outputs.iter().cloned())?;
        Ok(format!(
            "Inset {} face(s) on {} object(s)",
            self.faces.len(),
            self.sources.len()
        ))
    }
}
pub fn selected_targets(
    doc: &Document,
    faces: Option<&[usize]>,
) -> Result<Vec<(ObjectId, usize)>, CommandError> {
    let mut result = vec![];
    for object in doc.selected_objects() {
        let n = match object.geometry() {
            Geometry::SubD(s) => s.faces().len(),
            Geometry::Mesh(m) => m.face_count(),
            _ => return Err(CommandError::UnsupportedInsetGeometry),
        };
        let count = faces.map_or(n, <[usize]>::len);
        if result.len().saturating_add(count) > 1_000_000 {
            return Err(too_many_span_outputs("Inset"));
        }
        if let Some(faces) = faces {
            result.extend(faces.iter().map(|&f| (object.id(), f)));
        } else {
            result.extend((0..n).map(|f| (object.id(), f)));
        }
    }
    Ok(result)
}
#[derive(Clone, Copy)]
struct Defaults {
    subd: f64,
    mesh: f64,
}
impl Default for Defaults {
    fn default() -> Self {
        Self {
            subd: 0.1,
            mesh: 1.,
        }
    }
}
#[derive(Default)]
pub(super) struct InsetCommand(remembered::Remembered<Defaults>);
impl Command for InsetCommand {
    fn name(&self) -> &'static str {
        "Inset"
    }
    fn inset_options(&self, mesh: bool) -> Option<Options> {
        let d = self.0.get();
        Some(Options {
            distance: if mesh { d.mesh } else { d.subd },
            mode: Mode::Group,
        })
    }
    fn remember_inset_options(&self, options: Options, mesh: bool) -> bool {
        let mut d = self.0.get();
        if mesh {
            d.mesh = options.distance;
        } else {
            d.subd = options.distance;
        }
        self.0.set(d);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mesh = doc
            .selected_objects()
            .all(|o| matches!(o.geometry(), Geometry::Mesh(_)));
        let input = parse(args, self.inset_options(mesh).unwrap())?;
        if !input.faces_supplied {
            return Err(CommandError::Usage(
                "Inset on selected objects requires Faces=All or Faces=0,1,...; use the face-selection API otherwise",
            ));
        }
        let targets = selected_targets(doc, input.faces.as_deref())?;
        let plan = Plan::prepare(doc, targets, input.options)?;
        let message = plan.apply(doc)?;
        self.remember_inset_options(input.options, mesh);
        Ok(message)
    }
}
impl CommandRegistry {
    pub fn inset_options(&self, mesh: bool) -> Options {
        self.commands
            .iter()
            .find_map(|c| c.inset_options(mesh))
            .unwrap_or_default()
    }
    pub fn remember_inset_options(&self, options: Options, mesh: bool) {
        for c in &self.commands {
            if c.remember_inset_options(options, mesh) {
                break;
            }
        }
    }
}
