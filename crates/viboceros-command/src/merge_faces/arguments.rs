//! Typed selectors are script extensions; the interactive getter supplies targets.
use super::*;
#[derive(Clone, Debug)]
pub struct Parsed {
    pub results: Results,
    faces: Option<SurfaceFaceIndices>,
    edges: Option<SurfaceFaceIndices>,
    vertices: Option<SurfaceFaceIndices>,
    object: Option<ObjectId>,
}
impl Parsed {
    pub fn parse(args: &[&str], results: Results) -> Result<Self, CommandError> {
        let mut out = Self {
            results,
            faces: None,
            edges: None,
            vertices: None,
            object: None,
        };
        let mut seen_results = false;
        let mut i = 0;
        while i < args.len() {
            if option_name_eq(args[i], "Enter") && i + 1 == args.len() {
                break;
            }
            let (key, value, used) = if let Some((k, v)) = args[i].split_once('=') {
                (k, v, 1)
            } else {
                (
                    args[i],
                    *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                    2,
                )
            };
            if option_name_eq(key, "Results") && !seen_results {
                out.results = Results::parse(value).ok_or(CommandError::Usage(USAGE))?;
                seen_results = true;
            } else if option_name_eq(key, "Faces") && out.faces.is_none() {
                out.faces = Some(parse_surface_face_indices(value, USAGE)?);
            } else if option_name_eq(key, "Edges") && out.edges.is_none() {
                out.edges = Some(parse_surface_face_indices(value, USAGE)?);
            } else if option_name_eq(key, "Vertices") && out.vertices.is_none() {
                out.vertices = Some(parse_surface_face_indices(value, USAGE)?);
            } else if option_name_eq(key, "Object") && out.object.is_none() {
                out.object = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            i += used;
        }
        Ok(out)
    }
    pub fn component_getter(&self) -> bool {
        self.faces.is_none()
            && self.edges.is_none()
            && self.vertices.is_none()
            && self.object.is_none()
    }
    /// Resolve explicit script selectors against editable selected owners.
    pub fn targets(&self, document: &Document) -> Result<Vec<Target>, CommandError> {
        if self.object.is_some_and(|id| !document.is_selected(id)) {
            return Err(CommandError::MergeFacesUnavailable);
        }
        let mut targets = Vec::new();
        for object in document
            .selected_objects()
            .filter(|o| self.object.is_none_or(|id| id == o.id()))
        {
            let kinds = match object.geometry() {
                Geometry::Mesh(_) => [
                    ComponentSelectionKind::MeshFace,
                    ComponentSelectionKind::MeshEdge,
                    ComponentSelectionKind::MeshVertex,
                ],
                Geometry::SubD(_) => [
                    ComponentSelectionKind::SubDFace,
                    ComponentSelectionKind::SubDEdge,
                    ComponentSelectionKind::SubDVertex,
                ],
                _ => continue,
            };
            for (selector, kind) in [&self.faces, &self.edges, &self.vertices]
                .into_iter()
                .zip(kinds)
            {
                let Some(selector) = selector else { continue };
                let values = match selector {
                    SurfaceFaceIndices::Indices(v) => v.clone(),
                    SurfaceFaceIndices::All => {
                        let count = match (kind, object.geometry()) {
                            (ComponentSelectionKind::MeshFace, Geometry::Mesh(m)) => m.face_count(),
                            (ComponentSelectionKind::MeshEdge, Geometry::Mesh(m)) => {
                                m.topology_edge_points().len()
                            }
                            (ComponentSelectionKind::MeshVertex, Geometry::Mesh(m)) => {
                                m.topology_vertex_points().len()
                            }
                            (ComponentSelectionKind::SubDFace, Geometry::SubD(s)) => {
                                s.faces().len()
                            }
                            (ComponentSelectionKind::SubDEdge, Geometry::SubD(s)) => {
                                s.edges().len()
                            }
                            (ComponentSelectionKind::SubDVertex, Geometry::SubD(s)) => {
                                s.control_points().len()
                            }
                            _ => unreachable!(),
                        };
                        if count > 500_000usize.saturating_sub(targets.len()) {
                            return Err(CommandError::MergeFacesResourceLimit);
                        }
                        (0..count).collect()
                    }
                };
                if values.len() > 500_000usize.saturating_sub(targets.len()) {
                    return Err(CommandError::MergeFacesResourceLimit);
                }
                targets.extend(values.into_iter().map(|index| Target {
                    object: object.id(),
                    kind,
                    index,
                }));
            }
        }
        Ok(targets)
    }
}
