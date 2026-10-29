//! UI topology ordinals resolve to the geometry kernel's original raw identities.
use super::*;
use viboceros_geometry::{SubD, TriangleMesh};
fn bad() -> CommandError {
    CommandError::MergeFacesUnavailable
}
fn bits(p: Point3) -> [u64; 3] {
    p.to_array().map(|x| if x == 0. { 0 } else { x.to_bits() })
}
pub(super) fn mesh_components(
    mesh: &TriangleMesh,
    targets: &BTreeSet<Target>,
) -> Result<Vec<MeshMergeComponent>, CommandError> {
    let mut raw = BTreeMap::new();
    if targets
        .iter()
        .any(|t| t.kind != ComponentSelectionKind::MeshFace)
    {
        for (i, &p) in mesh.vertices().iter().enumerate() {
            raw.entry(bits(p)).or_insert(i);
        }
    }
    let vertices = targets
        .iter()
        .any(|t| t.kind == ComponentSelectionKind::MeshVertex)
        .then(|| mesh.topology_vertex_points());
    let edges = targets
        .iter()
        .any(|t| t.kind == ComponentSelectionKind::MeshEdge)
        .then(|| mesh.topology_edge_points());
    targets
        .iter()
        .map(|t| {
            Ok(match t.kind {
                ComponentSelectionKind::MeshFace if t.index < mesh.face_count() => {
                    MeshMergeComponent::Face(t.index)
                }
                ComponentSelectionKind::MeshVertex => {
                    let p = vertices
                        .as_ref()
                        .and_then(|v| v.get(t.index))
                        .ok_or_else(bad)?;
                    MeshMergeComponent::Vertex(raw[&bits(*p)])
                }
                ComponentSelectionKind::MeshEdge => {
                    let p = edges
                        .as_ref()
                        .and_then(|e| e.get(t.index))
                        .ok_or_else(bad)?;
                    MeshMergeComponent::Edge(p.map(|p| raw[&bits(p)]))
                }
                _ => return Err(bad()),
            })
        })
        .collect()
}
pub(super) fn subd_components(
    subd: &SubD,
    targets: &BTreeSet<Target>,
) -> Result<Vec<SubDMergeComponent>, CommandError> {
    targets
        .iter()
        .map(|t| {
            Ok(match t.kind {
                ComponentSelectionKind::SubDFace if t.index < subd.faces().len() => {
                    SubDMergeComponent::Face(t.index)
                }
                ComponentSelectionKind::SubDVertex if t.index < subd.control_points().len() => {
                    SubDMergeComponent::Vertex(t.index)
                }
                ComponentSelectionKind::SubDEdge => SubDMergeComponent::Edge(
                    subd.edges()
                        .get(t.index)
                        .ok_or_else(bad)?
                        .map(|i| i as usize),
                ),
                _ => return Err(bad()),
            })
        })
        .collect()
}
