//! Cached incidence and original-edge lineage through network subdivision.
use super::*;

#[derive(Default)]
pub(super) struct SourceEdges {
    faces: Vec<Vec<usize>>,
    original: Vec<bool>,
    owners: Vec<usize>,
}

impl SourceEdges {
    pub fn new(brep: &Brep) -> Self {
        let mut result = Self {
            faces: vec![Vec::new(); brep.edges.len()],
            original: vec![true; brep.edges.len()],
            owners: (0..brep.faces.len()).collect(),
        };
        result.add_faces(brep, 0..brep.faces.len());
        result
    }
    pub fn incidence(&self) -> &[Vec<usize>] {
        &self.faces
    }
    pub fn owner(&self, face: usize) -> usize {
        self.owners[face]
    }
    pub fn append_faces(&mut self, source: usize, count: usize) {
        let owner = self.owner(source);
        self.owners.extend(std::iter::repeat_n(owner, count));
    }
    pub fn reserve_edges(&mut self, count: usize) {
        self.faces.resize_with(count, Vec::new);
        self.original.resize(count, false);
    }
    pub fn flags(&self) -> Vec<bool> {
        self.original.clone()
    }
    pub fn inherit(
        &mut self,
        records: &[(usize, Vec<usize>)],
        edge_count: usize,
    ) -> Result<(), GeometryError> {
        self.reserve_edges(edge_count);
        for (old, fragments) in records {
            let faces = self
                .faces
                .get(*old)
                .cloned()
                .ok_or_else(|| invalid("inset subdivision is missing source-edge incidence"))?;
            let original = self.original[*old];
            for &fragment in fragments {
                self.faces[fragment] = faces.clone();
                self.original[fragment] = original;
            }
        }
        Ok(())
    }
    pub fn remove_face(&mut self, brep: &Brep, index: usize) {
        self.reserve_edges(brep.edges.len());
        for edge in brep.faces[index]
            .loops
            .iter()
            .flat_map(|l| &l.trims)
            .filter_map(|t| t.edge)
        {
            self.faces[edge].retain(|&f| f != index);
        }
    }
    pub fn add_faces(&mut self, brep: &Brep, indices: impl IntoIterator<Item = usize>) {
        self.reserve_edges(brep.edges.len());
        for index in indices {
            for edge in brep.faces[index]
                .loops
                .iter()
                .flat_map(|l| &l.trims)
                .filter_map(|t| t.edge)
            {
                if self.faces[edge].last() != Some(&index) {
                    self.faces[edge].push(index);
                }
            }
        }
    }
}
