//! Tensor-line barriers and oriented adjacency on the source control topology.
use super::*;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug)]
pub(super) struct Use {
    pub face: usize,
    pub side: usize,
}
pub(super) struct Graph<'a> {
    pub source: &'a SubD,
    pub face_edges: Vec<Vec<usize>>,
    pub uses: Vec<Vec<Use>>,
    pub vertex_edges: Vec<Vec<usize>>,
    pub walls: Vec<bool>,
}
impl<'a> Graph<'a> {
    pub fn new(source: &'a SubD) -> Result<Self, GeometryError> {
        let mut map = BTreeMap::new();
        let mut vertex_edges = vec![Vec::new(); source.control_points().len()];
        for (i, &[a, b]) in source.edges().iter().enumerate() {
            if map.insert((a.min(b), a.max(b)), i).is_some() {
                return Err(failure("Ambiguous parallel SubD packing edges"));
            }
            vertex_edges[a as usize].push(i);
            vertex_edges[b as usize].push(i);
        }
        let mut uses = vec![Vec::new(); source.edges().len()];
        let mut face_edges = Vec::with_capacity(source.faces().len());
        for (face, vertices) in source.faces().iter().enumerate() {
            let mut edges = Vec::with_capacity(vertices.len());
            for side in 0..vertices.len() {
                let (a, b) = (vertices[side], vertices[(side + 1) % vertices.len()]);
                let edge = *map
                    .get(&(a.min(b), a.max(b)))
                    .ok_or_else(|| failure("Missing SubD packing edge"))?;
                uses[edge].push(Use { face, side });
                edges.push(edge);
                if uses[edge].len() > 2 {
                    return Err(failure("SubD packing requires manifold edges"));
                }
            }
            face_edges.push(edges);
        }
        let mut graph = Self {
            source,
            face_edges,
            uses,
            vertex_edges,
            walls: Vec::new(),
        };
        graph.walls = (0..source.edges().len())
            .map(|e| {
                source.edge_tags()[e] == 2
                    || graph.uses[e].len() != 2
                    || graph.uses[e]
                        .iter()
                        .any(|u| source.faces()[u.face].len() != 4)
                    || source.edges()[e].iter().any(|&v| !graph.regular_end(e, v))
            })
            .collect();
        let mut queue = graph
            .walls
            .iter()
            .enumerate()
            .filter_map(|(i, &w)| w.then_some(i))
            .collect::<VecDeque<_>>();
        while let Some(edge) = queue.pop_front() {
            for vertex in source.edges()[edge] {
                if source.vertex_tags()[vertex as usize] != 1
                    || graph.vertex_edges[vertex as usize].len() != 4
                {
                    continue;
                }
                let adjacent = graph.uses[edge]
                    .iter()
                    .map(|&u| graph.other_edge(u, vertex))
                    .collect::<Vec<_>>();
                let opposite = graph.vertex_edges[vertex as usize]
                    .iter()
                    .copied()
                    .filter(|&e| e != edge && !adjacent.contains(&e))
                    .collect::<Vec<_>>();
                if let [opposite] = opposite.as_slice()
                    && !graph.walls[*opposite]
                {
                    graph.walls[*opposite] = true;
                    queue.push_back(*opposite);
                }
            }
        }
        Ok(graph)
    }
    fn other_edge(&self, u: Use, vertex: u32) -> usize {
        let face = &self.source.faces()[u.face];
        self.face_edges[u.face][if face[u.side] == vertex {
            (u.side + face.len() - 1) % face.len()
        } else {
            (u.side + 1) % face.len()
        }]
    }
    fn regular_end(&self, edge: usize, vertex: u32) -> bool {
        match self.source.vertex_tags()[vertex as usize] {
            1 => self.vertex_edges[vertex as usize].len() == 4,
            2 => self.uses[edge]
                .iter()
                .all(|&u| self.source.edge_tags()[self.other_edge(u, vertex)] == 2),
            _ => false,
        }
    }
    pub fn components(&self) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.source.faces().len()];
        let mut groups = Vec::new();
        for root in 0..seen.len() {
            if seen[root] {
                continue;
            }
            seen[root] = true;
            let mut group = vec![root];
            let mut cursor = 0;
            while cursor < group.len() {
                for &edge in &self.face_edges[group[cursor]] {
                    if self.walls[edge] {
                        continue;
                    }
                    for u in &self.uses[edge] {
                        if !seen[u.face] {
                            seen[u.face] = true;
                            group.push(u.face);
                        }
                    }
                }
                cursor += 1;
            }
            groups.push(group);
        }
        groups
    }
    pub fn neighbor(
        &self,
        cell: Cell,
        direction: usize,
        allowed: &std::collections::HashSet<usize>,
        stored: bool,
    ) -> Option<Cell> {
        let side = (direction + 1 + 4 - cell.rotation as usize) % 4;
        let edge = self.face_edges[cell.face][side];
        if self.source.edge_tags()[edge] == 2 || (!stored && self.walls[edge]) {
            return None;
        }
        self.physical(cell, direction, allowed)
    }

    pub fn physical(
        &self,
        cell: Cell,
        direction: usize,
        allowed: &std::collections::HashSet<usize>,
    ) -> Option<Cell> {
        let side = (direction + 1 + 4 - cell.rotation as usize) % 4;
        let edge = self.face_edges[cell.face][side];
        let other = self.uses[edge].iter().find(|u| u.face != cell.face)?;
        if !allowed.contains(&other.face) || self.source.faces()[other.face].len() != 4 {
            return None;
        }
        let grid_side = (side + cell.rotation as usize) % 4;
        Some(Cell {
            face: other.face,
            rotation: ((grid_side + 6 - other.side) % 4) as u8,
        })
    }
}
