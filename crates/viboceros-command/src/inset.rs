//! Transactional face insets on polygonal surfaces, SubDs and stored mesh faces.
use crate::*;
mod arguments;
mod command;
pub(super) use command::InsetCommand;
mod surface;
mod witness;
pub use witness::Witness;
#[cfg(test)]
mod tests;
pub use arguments::{Input, Mode, Options, parse};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub object: ObjectId,
    pub index: usize,
    pub kind: ComponentSelectionKind,
}
#[derive(Clone, Debug)]
pub struct Plan {
    witness: Witness,
    outputs: Vec<(ObjectId, Geometry)>,
    faces: Vec<Face>,
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
        let surface_count = grouped
            .keys()
            .filter(|id| {
                doc.object(**id)
                    .is_some_and(|o| surface::is_surface(o.geometry()))
            })
            .count();
        if surface_count != 0 && surface_count != grouped.len() {
            return Err(CommandError::Usage(
                "Inset surface faces separately from mesh and SubD faces",
            ));
        }
        let witness = Witness::capture(doc, grouped.keys().copied())?;
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
                Geometry::Brep(_) | Geometry::NurbsSurface(_) => {
                    surface::prepare(object.geometry(), &indices, options, doc.tolerance())?
                }
                Geometry::SubD(s) => {
                    surface::require_polygon_options(options)?;
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
                    surface::require_polygon_options(options)?;
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
        }
        Ok(Self {
            witness,
            outputs,
            faces,
        })
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.witness.is_current(doc)
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
                Geometry::Brep(b) => {
                    if b.edges()
                        .len()
                        .saturating_mul(8)
                        .saturating_add(lines.len())
                        > 400_000
                    {
                        return Err(too_many_span_outputs("Inset"));
                    }
                    let mut pairs = Vec::new();
                    for edge in b.edges() {
                        let curve = edge.curve();
                        let count = if curve.degree() == 1 && curve.control_points().len() == 2 {
                            1
                        } else {
                            8
                        };
                        let mut a = curve.evaluate(*curve.domain().start())?;
                        for i in 1..=count {
                            let b = curve.evaluate(curve.parameter_at(i as f64 / count as f64)?)?;
                            pairs.push([a, b]);
                            a = b;
                        }
                    }
                    pairs
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
            self.witness.len()
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
            Geometry::Brep(b) => b.faces().len(),
            Geometry::NurbsSurface(_) => 1,
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
