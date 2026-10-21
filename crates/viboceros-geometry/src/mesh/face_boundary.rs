//! Incremental exact-location boundary incidence with bounded per-face undo logs.
use super::*;

type Key = [u64; 3];
type EdgeKey = [Key; 2];
#[derive(Clone, Copy, Debug)]
struct Edge {
    count: usize,
    points: [Point3; 2],
    normal: Option<UnitVector3>,
}
/// Directed naked edges of source and staged faces. Appending a face changes
/// only its incident edges; a checkpoint restores these changes in reverse.
#[derive(Clone, Debug, Default)]
pub struct MeshFaceBoundaryGraph {
    edges: BTreeMap<EdgeKey, Edge>,
    outgoing: BTreeMap<Key, BTreeSet<Key>>,
    incoming: BTreeMap<Key, BTreeSet<Key>>,
    journal: Vec<(EdgeKey, Option<Edge>)>,
}
fn key(point: Point3) -> Key {
    point.to_array().map(canonical_coordinate_bits)
}
fn pair([a, b]: [Point3; 2]) -> EdgeKey {
    let [a, b] = [key(a), key(b)];
    if a < b { [a, b] } else { [b, a] }
}
impl MeshFaceBoundaryGraph {
    pub fn from_mesh(mesh: &TriangleMesh) -> Result<Self, GeometryError> {
        let mut graph = Self::default();
        graph.append(mesh)?;
        graph.journal.clear();
        Ok(graph)
    }
    pub fn checkpoint(&self) -> usize {
        self.journal.len()
    }
    /// Validate the normal arithmetic before changing incidence. A zero normal
    /// remains None, so its boundary cannot invent a continuation frame.
    pub fn append(&mut self, mesh: &TriangleMesh) -> Result<(), GeometryError> {
        let normals = (0..mesh.faces().len())
            .map(|i| match mesh.polygon_face_normal(i) {
                Ok(n) => Ok(Some(n)),
                Err(GeometryError::Degenerate { context: "vector" }) => Ok(None),
                Err(e) => Err(e),
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        for (face, normal) in mesh.faces().iter().zip(normals) {
            let indices = face.indices();
            for i in 0..indices.len() {
                let points = [
                    mesh.vertices()[indices[i] as usize],
                    mesh.vertices()[indices[(i + 1) % indices.len()] as usize],
                ];
                if points[0] == points[1] {
                    continue;
                }
                let edge_key = pair(points);
                let previous = self.edges.get(&edge_key).copied();
                let edge = if let Some(mut edge) = previous {
                    edge.count += 1;
                    edge
                } else {
                    Edge {
                        count: 1,
                        points,
                        normal,
                    }
                };
                self.journal.push((edge_key, previous));
                self.replace(edge_key, Some(edge));
            }
        }
        Ok(())
    }
    pub fn rollback(&mut self, checkpoint: usize) -> Result<(), GeometryError> {
        if checkpoint > self.journal.len() {
            return Err(GeometryError::Degenerate {
                context: "invalid mesh boundary checkpoint",
            });
        }
        while self.journal.len() > checkpoint {
            let (key, previous) = self.journal.pop().unwrap();
            self.replace(key, previous);
        }
        Ok(())
    }
    pub fn boundary_normal(&self, points: [Point3; 2]) -> Option<UnitVector3> {
        self.edges
            .get(&pair(points))
            .filter(|edge| edge.count == 1)
            .and_then(|edge| edge.normal)
    }
    /// Return the unique outgoing/incoming boundary use at an exact point.
    /// The seed runs opposite the incident face; branches remain explicit.
    pub fn next_seed(&self, point: Point3, forward: bool) -> Option<([Point3; 2], UnitVector3)> {
        let here = key(point);
        let map = if forward {
            &self.outgoing
        } else {
            &self.incoming
        };
        let neighbors = map.get(&here)?;
        if neighbors.len() != 1 {
            return None;
        }
        let there = *neighbors.first()?;
        let edge_key = if here < there {
            [here, there]
        } else {
            [there, here]
        };
        let edge = self.edges.get(&edge_key)?;
        Some(([edge.points[1], edge.points[0]], edge.normal?))
    }
    fn replace(&mut self, edge_key: EdgeKey, next: Option<Edge>) {
        if let Some(previous) = self.edges.remove(&edge_key).filter(|edge| edge.count == 1) {
            self.adjacency(previous, false);
        }
        if let Some(edge) = next {
            self.edges.insert(edge_key, edge);
            if edge.count == 1 {
                self.adjacency(edge, true);
            }
        }
    }
    fn adjacency(&mut self, edge: Edge, insert: bool) {
        let [a, b] = edge.points.map(key);
        for (map, from, to) in [(&mut self.outgoing, a, b), (&mut self.incoming, b, a)] {
            if insert {
                map.entry(from).or_default().insert(to);
            } else if let Some(neighbors) = map.get_mut(&from) {
                neighbors.remove(&to);
                if neighbors.is_empty() {
                    map.remove(&from);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    fn quad(x: Real) -> TriangleMesh {
        TriangleMesh::try_face_from_points(
            &[p(x, 0.), p(x + 1., 0.), p(x + 1., 1.), p(x, 1.)],
            false,
        )
        .unwrap()
    }
    #[test]
    fn append_and_rollback_restore_the_source_boundary_and_remove_new_adjacency() {
        let source = quad(0.);
        let mut graph = MeshFaceBoundaryGraph::from_mesh(&source).unwrap();
        let checkpoint = graph.checkpoint();
        let before = graph.next_seed(p(1., 1.), true).unwrap();
        graph.append(&quad(1.)).unwrap();
        assert!(graph.boundary_normal([p(1., 0.), p(1., 1.)]).is_none());
        assert!(graph.next_seed(p(2., 1.), true).is_some());
        graph.rollback(checkpoint).unwrap();
        assert_eq!(graph.next_seed(p(1., 1.), true), Some(before));
        assert!(graph.next_seed(p(2., 1.), true).is_none());
        assert!(graph.boundary_normal([p(1., 0.), p(1., 1.)]).is_some());
        assert!(graph.rollback(checkpoint + 1).is_err());
    }
    #[test]
    fn a_range_error_is_atomic_and_large_strips_charge_only_incident_edges() {
        let mut graph = MeshFaceBoundaryGraph::default();
        let invalid = TriangleMesh::try_from_face_records(
            vec![p(-1e308, 0.), p(1e308, 0.), p(0., 1.)],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        assert!(graph.append(&invalid).is_err());
        assert!(graph.edges.is_empty());
        assert_eq!(graph.checkpoint(), 0);
        for i in 0..1000 {
            graph.append(&quad(i as Real)).unwrap();
        }
        assert_eq!(graph.checkpoint(), 4000);
        assert_eq!(graph.edges.len(), 3001);
        assert_eq!(
            graph.outgoing.values().map(BTreeSet::len).sum::<usize>(),
            2002
        );
        graph.rollback(0).unwrap();
        assert!(graph.edges.is_empty());
        assert!(graph.outgoing.is_empty());
        assert!(graph.incoming.is_empty());
    }

    #[test]
    fn undefined_normals_are_retained_without_fabricating_a_boundary_frame() {
        let mesh = TriangleMesh::try_face_from_points(
            &[p(4., 3.), p(0., 0.), p(0., 3.), p(4., 0.)],
            false,
        )
        .unwrap();
        let mut graph = MeshFaceBoundaryGraph::default();
        graph.append(&mesh).unwrap();
        assert_eq!(graph.checkpoint(), 4);
        assert_eq!(graph.edges.len(), 4);
        assert!(graph.boundary_normal([p(4., 3.), p(0., 0.)]).is_none());
        assert!(graph.next_seed(p(4., 3.), true).is_none());
        graph.rollback(0).unwrap();
        assert!(graph.edges.is_empty());
    }
}
