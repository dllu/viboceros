//! Linear-time edge-incidence classification without repeated trim collection.

use super::Brep;

impl Brep {
    /// Exact trim-use counts in edge-index order, computed in one traversal.
    /// Prefer this to repeated `edge_use_count` calls when inspecting all edges.
    /// Singular trims have no edge and do not contribute.
    pub fn edge_use_counts(&self) -> Vec<usize> {
        let mut counts = vec![0; self.edges.len()];
        for face in &self.faces {
            for boundary in &face.loops {
                for trim in &boundary.trims {
                    if let Some(edge) = trim.edge {
                        counts[edge] += 1;
                    }
                }
            }
        }
        counts
    }

    /// Exact use count for one edge, ignoring singular trims with no edge.
    pub fn edge_use_count(&self, edge_index: usize) -> Option<usize> {
        (edge_index < self.edges.len()).then(|| {
            self.faces
                .iter()
                .flat_map(|face| &face.loops)
                .flat_map(|boundary| &boundary.trims)
                .filter(|trim| trim.edge == Some(edge_index))
                .count()
        })
    }

    pub fn is_manifold(&self) -> bool {
        self.edge_incidence()
            .iter()
            .all(|counts| counts[0] + counts[1] <= 2)
    }

    pub fn is_closed(&self) -> bool {
        self.edge_incidence()
            .iter()
            .all(|counts| counts[0] + counts[1] == 2)
    }

    pub fn is_solid(&self) -> bool {
        self.edge_incidence().iter().all(|&counts| counts == [1, 1])
    }

    // Two bytes per edge. Each oriented count saturates at three, since all
    // predicates distinguish only zero, one, two, and more-than-two uses.
    // Topology validation guarantees that every referenced edge exists.
    fn edge_incidence(&self) -> Vec<[u8; 2]> {
        let mut counts = vec![[0u8; 2]; self.edges.len()];
        for face in &self.faces {
            for boundary in &face.loops {
                for trim in &boundary.trims {
                    if let Some(edge) = trim.edge {
                        let count =
                            &mut counts[edge][usize::from(trim.reversed_3d ^ face.reversed)];
                        *count = (*count + 1).min(3);
                    }
                }
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Tolerance, TriangleMesh};

    #[test]
    fn incidence_queries_match_explicit_edge_use_enumeration() {
        let vertices = [
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0., -1., 1.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
        let tetra = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let mut hanging = tetra.clone();
        hanging.push([0, 1, 4]);
        for faces in [vec![[0, 2, 1]], tetra, hanging, vec![[0, 2, 1]; 300]] {
            let mesh = TriangleMesh::try_new(vertices.clone(), faces, Tolerance::DEFAULT).unwrap();
            for trimmed in [false, true] {
                let source = Brep::try_from_mesh(&mesh, trimmed, Tolerance::DEFAULT).unwrap();
                let mut flipped_face = source.clone();
                flipped_face.faces[0].reversed = !flipped_face.faces[0].reversed;
                for brep in [source.clone(), source.reversed(), flipped_face] {
                    let uses = brep.trim_uses();
                    let counts = brep.edge_use_counts();
                    assert_eq!(counts.len(), brep.edges.len());
                    let mut expected_manifold = true;
                    let mut expected_closed = true;
                    let mut expected_solid = true;
                    for (edge, &count) in counts.iter().enumerate() {
                        let edge_uses = uses
                            .iter()
                            .filter(|usage| usage.trim.edge == Some(edge))
                            .collect::<Vec<_>>();
                        assert_eq!(brep.edge_use_count(edge), Some(edge_uses.len()));
                        assert_eq!(count, edge_uses.len());
                        expected_manifold &= edge_uses.len() <= 2;
                        expected_closed &= edge_uses.len() == 2;
                        expected_solid &= edge_uses.len() == 2
                            && (edge_uses[0].trim.reversed_3d
                                ^ brep.faces[edge_uses[0].face].reversed)
                                != (edge_uses[1].trim.reversed_3d
                                    ^ brep.faces[edge_uses[1].face].reversed);
                    }
                    assert_eq!(brep.is_manifold(), expected_manifold);
                    assert_eq!(brep.is_closed(), expected_closed);
                    assert_eq!(brep.is_solid(), expected_solid);
                    assert_eq!(brep.edge_use_count(brep.edges.len()), None);
                    assert_eq!(brep.edge_use_count(usize::MAX), None);
                }
            }
        }
    }
}
