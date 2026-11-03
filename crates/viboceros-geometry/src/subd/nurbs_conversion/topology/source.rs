//! Source component keys and original/sector edge segmentation.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Vertex {
    Control(u32),
    Midpoint(usize),
    Center(usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Edge {
    Control(usize, u8),
    Sector(usize, usize),
}
pub(super) struct Side {
    pub(super) edge: Edge,
    pub(super) vertices: [Vertex; 2],
    pub(super) interval: [f64; 2],
    pub(super) mated: bool,
}

pub(super) struct SourceEdges {
    faces: Vec<Vec<usize>>,
    split: Vec<bool>,
}
impl SourceEdges {
    pub(super) fn new(source: &SubD) -> Result<Self, GeometryError> {
        let mut map = BTreeMap::new();
        for (index, &[a, b]) in source.edges().iter().enumerate() {
            if map.insert((a.min(b), a.max(b)), index).is_some() {
                return Err(failure("ambiguous parallel SubD control edges"));
            }
        }
        if source.edge_face_counts().iter().any(|&n| n > 2) {
            return Err(failure("nonmanifold SubD patch assembly"));
        }
        let mut split = vec![false; source.edges().len()];
        let faces = source
            .faces()
            .iter()
            .map(|face| {
                (0..face.len())
                    .map(|j| {
                        let (a, b) = (face[j], face[(j + 1) % face.len()]);
                        let e = *map
                            .get(&(a.min(b), a.max(b)))
                            .ok_or_else(|| failure("missing SubD control edge"))?;
                        split[e] |= face.len() != 4;
                        Ok(e)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { faces, split })
    }

    pub(super) fn sides(
        &self,
        source: &SubD,
        face_index: usize,
        corner: Option<usize>,
        side: usize,
    ) -> Vec<Side> {
        let face = &source.faces()[face_index];
        let edges = &self.faces[face_index];
        let control = |edge: usize, vertices: [Vertex; 2], interval| {
            let part = if !self.split[edge] {
                2
            } else if vertices.contains(&Vertex::Control(source.edges()[edge][0])) {
                0
            } else {
                1
            };
            Side {
                edge: Edge::Control(edge, part),
                vertices,
                interval,
                mated: source.edge_face_counts()[edge] == 2,
            }
        };
        if let Some(corner) = corner {
            let previous = (corner + face.len() - 1) % face.len();
            let outgoing = edges[corner];
            let incoming = edges[previous];
            let v = Vertex::Control(face[corner]);
            let center = Vertex::Center(face_index);
            let next = Vertex::Midpoint(outgoing);
            let prev = Vertex::Midpoint(incoming);
            vec![match side {
                0 => control(outgoing, [v, next], [0., 1.]),
                1 => Side {
                    edge: Edge::Sector(face_index, corner),
                    vertices: [next, center],
                    interval: [0., 1.],
                    mated: true,
                },
                2 => Side {
                    edge: Edge::Sector(face_index, previous),
                    vertices: [center, prev],
                    interval: [0., 1.],
                    mated: true,
                },
                _ => control(incoming, [prev, v], [0., 1.]),
            }]
        } else {
            let edge = edges[side];
            let a = Vertex::Control(face[side]);
            let b = Vertex::Control(face[(side + 1) % 4]);
            if self.split[edge] {
                let middle = Vertex::Midpoint(edge);
                vec![
                    control(edge, [a, middle], [0., 0.5]),
                    control(edge, [middle, b], [0.5, 1.]),
                ]
            } else {
                vec![control(edge, [a, b], [0., 1.])]
            }
        }
    }
}
