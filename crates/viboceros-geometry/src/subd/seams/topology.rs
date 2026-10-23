use super::*;
pub(super) fn seam_error(message: &str) -> GeometryError {
    viboceros_opennurbs::SubDError(message.into()).into()
}
pub(super) fn selection(subd: &SubD, edges: &[usize]) -> Result<BTreeSet<usize>, GeometryError> {
    if edges.iter().any(|&i| i >= subd.edges().len()) {
        return Err(seam_error("SubD seam edge outside its source table"));
    }
    Ok(edges.iter().copied().collect())
}
pub(super) struct Topology {
    pub vertices: Vec<u32>,
    pub offsets: Vec<usize>,
    pub uses: Vec<Vec<[usize; 2]>>,
}
impl Topology {
    pub fn new(subd: &SubD) -> Result<Self, GeometryError> {
        if subd.edge_face_counts().iter().any(|&n| n == 0 || n > 2) {
            return Err(seam_error("SubD seam edits require manifold face edges"));
        }
        let mut lookup = HashMap::new();
        for (i, &[a, b]) in subd.edges().iter().enumerate() {
            if lookup.insert((a.min(b), a.max(b)), i).is_some() {
                return Err(seam_error(
                    "Parallel SubD source edges need explicit face-edge identities",
                ));
            }
        }
        let mut uses = vec![Vec::new(); subd.edges().len()];
        let mut offsets = Vec::new();
        let mut vertices = Vec::new();
        for face in subd.faces() {
            let offset = vertices.len();
            offsets.push(offset);
            vertices.extend_from_slice(face);
            for (i, &a) in face.iter().enumerate() {
                let j = (i + 1) % face.len();
                let b = face[j];
                let &edge = lookup
                    .get(&(a.min(b), a.max(b)))
                    .ok_or_else(|| seam_error("Missing SubD face edge"))?;
                let pair = [offset + i, offset + j];
                uses[edge].push(if subd.edges()[edge][0] == a {
                    pair
                } else {
                    [pair[1], pair[0]]
                });
            }
        }
        if uses
            .iter()
            .zip(subd.edge_face_counts())
            .any(|(u, &n)| u.len() != n as usize)
        {
            return Err(seam_error(
                "SubD face incidence differs from its edge table",
            ));
        }
        Ok(Self {
            vertices,
            offsets,
            uses,
        })
    }
}
