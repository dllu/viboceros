//! Ordered opposite-edge walks shared by mesh topology and SubD control nets.
use crate::GeometryError;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolygonEdgeRing {
    /// Source edge ordinals, with no repeated closing edge.
    pub edges: Vec<usize>,
    /// Source faces crossed between successive edges, including the closing face.
    pub faces: Vec<usize>,
    closed: bool,
}
impl PolygonEdgeRing {
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

/// Prepared incidence reused for many seeds on the same immutable source.
#[derive(Clone, Debug)]
pub struct PolygonEdgeRingIndex {
    quads: Vec<Option<[usize; 4]>>,
    incident: Vec<Vec<usize>>,
}
impl PolygonEdgeRingIndex {
    pub(crate) fn new(edges: &[[usize; 2]], faces: impl IntoIterator<Item = Vec<usize>>) -> Self {
        let key = |[a, b]: [usize; 2]| if a < b { (a, b) } else { (b, a) };
        let ordinals = edges
            .iter()
            .enumerate()
            .map(|(i, &e)| (key(e), i))
            .collect::<BTreeMap<_, _>>();
        let mut incident = vec![Vec::new(); edges.len()];
        let quads = faces
            .into_iter()
            .enumerate()
            .map(|(face, vertices)| {
                let mut sides = Vec::with_capacity(vertices.len());
                for i in 0..vertices.len() {
                    if let Some(&edge) =
                        ordinals.get(&key([vertices[i], vertices[(i + 1) % vertices.len()]]))
                    {
                        incident[edge].push(face);
                        sides.push(edge);
                    }
                }
                (vertices.len() == 4
                    && vertices.iter().collect::<BTreeSet<_>>().len() == 4
                    && sides.len() == 4)
                    .then(|| <[usize; 4]>::try_from(sides).unwrap())
            })
            .collect::<Vec<_>>();
        Self { quads, incident }
    }
    pub fn edge_count(&self) -> usize {
        self.incident.len()
    }
    pub fn try_ring(&self, seed: usize) -> Result<PolygonEdgeRing, GeometryError> {
        let (quads, incident) = (&self.quads, &self.incident);
        if seed >= incident.len() {
            return Err(GeometryError::Degenerate {
                context: "ring seed outside topology",
            });
        }
        if incident[seed].len() > 2 {
            return Ok(PolygonEdgeRing {
                edges: Vec::new(),
                faces: Vec::new(),
                closed: false,
            });
        }
        let mut seen = BTreeSet::from([seed]);
        let mut crossed = BTreeSet::new();
        let mut closed = false;
        let mut extend = |face: usize| {
            let (mut current, mut face) = (seed, face);
            let mut path = Vec::new();
            let mut crossed_faces = Vec::new();
            loop {
                if incident[current].len() > 2 || !crossed.insert(face) {
                    break;
                }
                let Some(quad) = quads[face] else { break };
                let at = quad.iter().position(|&e| e == current).unwrap();
                let next = quad[(at + 2) % 4];
                if incident[next].len() > 2 {
                    break;
                }
                if next == seed {
                    crossed_faces.push(face);
                    closed = true;
                    break;
                }
                if !seen.insert(next) {
                    break;
                }
                path.push(next);
                crossed_faces.push(face);
                let Some(&other) = incident[next].iter().find(|&&f| f != face) else {
                    break;
                };
                current = next;
                face = other;
            }
            (path, crossed_faces)
        };
        let (after, after_faces) = incident[seed]
            .first()
            .map(|&f| extend(f))
            .unwrap_or_default();
        let (mut before, mut before_faces) = incident[seed]
            .get(1)
            .map(|&f| extend(f))
            .unwrap_or_default();
        before.reverse();
        before.push(seed);
        before.extend(after);
        before_faces.reverse();
        before_faces.extend(after_faces);
        Ok(PolygonEdgeRing {
            edges: before,
            faces: before_faces,
            closed,
        })
    }
}
