//! Exact control-topology seams retain native IDs, sharpness and face properties.
use super::*;
use crate::mesh::union_find::{index_root, union_faces};
use std::collections::{BTreeMap, BTreeSet, HashMap};
mod topology;
use topology::*;

#[derive(Clone, Debug)]
pub struct SubDEdgeSeamResult {
    pub subd: SubD,
    pub changed_edges: usize,
    /// Every input control-edge ordinal maps to its surviving output edge.
    pub edge_map: Vec<usize>,
}
impl SubD {
    fn unchanged_seams(&self) -> SubDEdgeSeamResult {
        SubDEdgeSeamResult {
            subd: self.clone(),
            changed_edges: 0,
            edge_map: (0..self.edges().len()).collect(),
        }
    }
}

impl SubD {
    /// Split selected manifold control edges wherever radial face fans separate.
    /// Boundaries and isolated cuts on a closed fan make no topology change.
    /// Original control indices remain valid; new controls are appended.
    pub fn try_unweld_edges(&self, edges: &[usize]) -> Result<(Self, usize), GeometryError> {
        self.try_unweld_edges_mapped(edges)
            .map(|r| (r.subd, r.changed_edges))
    }
    pub fn try_unweld_edges_mapped(
        &self,
        edges: &[usize],
    ) -> Result<SubDEdgeSeamResult, GeometryError> {
        let selected = selection(self, edges)?;
        if selected.is_empty() {
            return Ok(self.unchanged_seams());
        }
        let topology = Topology::new(self)?;
        let mut parents = (0..topology.vertices.len()).collect::<Vec<_>>();
        let mut ranks = vec![0; parents.len()];
        for (edge, uses) in topology.uses.iter().enumerate() {
            if selected.contains(&edge) {
                continue;
            }
            if let Some(first) = uses.first() {
                for next in &uses[1..] {
                    for end in 0..2 {
                        union_faces(&mut parents, &mut ranks, first[end], next[end]);
                    }
                }
            }
        }
        let mut groups = vec![BTreeMap::<usize, usize>::new(); self.control_points().len()];
        for (corner, &v) in topology.vertices.iter().enumerate() {
            let root = index_root(&mut parents, corner);
            groups[v as usize].entry(root).or_insert(corner);
        }
        let mut roots = groups
            .into_iter()
            .map(|g| {
                let mut rows = g.into_iter().collect::<Vec<_>>();
                rows.sort_by_key(|&(_, corner)| corner);
                rows.into_iter().map(|(root, _)| root).collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut assigned = vec![usize::MAX; parents.len()];
        for (vertex, groups) in roots.iter_mut().enumerate() {
            if !groups.is_empty() {
                assigned[groups.remove(0)] = vertex;
            }
        }
        let mut next = vec![0; roots.len()];
        let mut copies = Vec::new();
        let mut allocate = |vertex: usize| {
            if next[vertex] < roots[vertex].len() {
                assigned[roots[vertex][next[vertex]]] = self.control_points().len() + copies.len();
                copies.push(vertex);
                next[vertex] += 1;
                true
            } else {
                false
            }
        };
        let interior = selected
            .iter()
            .copied()
            .filter(|&e| self.edge_face_counts()[e] == 2)
            .collect::<Vec<_>>();
        for mut chain in self.try_edge_chains(&interior)? {
            if chain.is_closed() {
                chain.vertices.pop();
                let start = chain
                    .vertices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| *v)
                    .unwrap()
                    .0;
                chain.vertices.rotate_left(start);
            }
            for v in chain.vertices {
                allocate(v);
            }
        }
        for vertex in 0..roots.len() {
            while allocate(vertex) {}
        }
        if copies.is_empty() {
            return Ok(self.unchanged_seams());
        }
        let faces = self
            .faces()
            .iter()
            .enumerate()
            .map(|(f, face)| {
                (0..face.len())
                    .map(|i| assigned[index_root(&mut parents, topology.offsets[f] + i)] as u32)
                    .collect()
            })
            .collect::<Vec<Vec<u32>>>();
        let representatives = (0..self.edges().len()).collect::<Vec<_>>();
        let (data, changed) = viboceros_opennurbs::subd_remap_vertices(
            self.archive(),
            &copies,
            &[],
            &representatives,
            &faces,
            false,
        )?;
        Self::from_native(data).map(|subd| SubDEdgeSeamResult {
            subd,
            changed_edges: changed,
            edge_map: representatives,
        })
    }

    /// Join exact coincident boundary edges within this SubD. One selected side
    /// admits its unique opposite side. Vertex-only contacts remain separate.
    pub fn try_weld_edges(&self, edges: &[usize]) -> Result<(Self, usize), GeometryError> {
        self.try_weld_edges_mapped(edges)
            .map(|r| (r.subd, r.changed_edges))
    }
    pub fn try_weld_edges_mapped(
        &self,
        edges: &[usize],
    ) -> Result<SubDEdgeSeamResult, GeometryError> {
        let selected = selection(self, edges)?;
        if selected.is_empty() {
            return Ok(self.unchanged_seams());
        }
        let _ = Topology::new(self)?;
        type Location = [u64; 3];
        let location = |v: u32| -> Location {
            self.control_points()[v as usize]
                .to_array()
                .map(|x| if x == 0. { 0 } else { x.to_bits() })
        };
        let mut coincident = BTreeMap::<[Location; 2], Vec<usize>>::new();
        for (i, &[a, b]) in self.edges().iter().enumerate() {
            if self.edge_face_counts()[i] == 1 {
                let mut key = [location(a), location(b)];
                key.sort();
                coincident.entry(key).or_default().push(i);
            }
        }
        let mut vertices = (0..self.control_points().len()).collect::<Vec<_>>();
        let mut pairs = Vec::new();
        for group in coincident.values() {
            if !group.iter().any(|i| selected.contains(i)) || group.len() < 2 {
                continue;
            }
            if group.len() != 2 {
                return Err(seam_error("Ambiguous coincident SubD boundary edges"));
            }
            let keep = *group.iter().find(|i| selected.contains(i)).unwrap();
            pairs.push((keep, if keep == group[0] { group[1] } else { group[0] }));
        }
        if pairs.is_empty() {
            return Ok(self.unchanged_seams());
        }
        pairs.sort_unstable();
        for (keep, other) in pairs {
            let a = self.edges()[keep];
            let b = self.edges()[other];
            let b = if location(a[0]) == location(b[0]) {
                b
            } else {
                [b[1], b[0]]
            };
            for end in 0..2 {
                let keep = index_root(&mut vertices, a[end] as usize);
                let other = index_root(&mut vertices, b[end] as usize);
                vertices[other] = keep;
            }
        }
        let retired = (0..vertices.len())
            .filter(|&i| index_root(&mut vertices, i) != i)
            .collect::<Vec<_>>();
        let mut representatives = (0..self.edges().len()).collect::<Vec<_>>();
        let mut joined = HashMap::<_, Vec<usize>>::new();
        for (i, &[a, b]) in self.edges().iter().enumerate() {
            let a = index_root(&mut vertices, a as usize);
            let b = index_root(&mut vertices, b as usize);
            let key = (a.min(b), a.max(b));
            joined.entry(key).or_default().push(i);
        }
        for group in joined.values() {
            let keep = *group
                .iter()
                .find(|i| selected.contains(i))
                .unwrap_or(&group[0]);
            for &i in group {
                representatives[i] = keep;
            }
        }
        let faces = self
            .faces()
            .iter()
            .map(|face| {
                face.iter()
                    .map(|&v| index_root(&mut vertices, v as usize) as u32)
                    .collect()
            })
            .collect::<Vec<Vec<u32>>>();
        // The topology read also rejects ambiguous parallel source edges.
        let (data, changed) = viboceros_opennurbs::subd_remap_vertices(
            self.archive(),
            &[],
            &retired,
            &representatives,
            &faces,
            true,
        )?;
        let mut surviving = vec![usize::MAX; representatives.len()];
        let mut ordinal = 0;
        for (i, &rep) in representatives.iter().enumerate() {
            if rep == i {
                surviving[i] = ordinal;
                ordinal += 1;
            }
        }
        let edge_map = representatives.iter().map(|&rep| surviving[rep]).collect();
        Self::from_native(data).map(|subd| SubDEdgeSeamResult {
            subd,
            changed_edges: changed,
            edge_map,
        })
    }
}
