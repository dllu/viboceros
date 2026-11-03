//! Sew by source component identity, including half-edges beside n-gons.
use super::*;
use crate::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve, NurbsCurve2, Point2, SurfaceIso, Tolerance,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Vertex {
    Control(u32),
    Midpoint(usize),
    Center(usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Edge {
    Control(usize, u8),
    Sector(usize, usize),
}
struct Side {
    edge: Edge,
    vertices: [Vertex; 2],
    interval: [f64; 2],
    mated: bool,
}

struct SourceEdges {
    faces: Vec<Vec<usize>>,
    split: Vec<bool>,
}
impl SourceEdges {
    fn new(source: &SubD) -> Result<Self, GeometryError> {
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

    fn sides(&self, source: &SubD, patch: &SubDNurbsPatch, side: usize) -> Vec<Side> {
        let face = &source.faces()[patch.face];
        let edges = &self.faces[patch.face];
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
        if let Some(corner) = patch.corner {
            let previous = (corner + face.len() - 1) % face.len();
            let outgoing = edges[corner];
            let incoming = edges[previous];
            let v = Vertex::Control(face[corner]);
            let center = Vertex::Center(patch.face);
            let next = Vertex::Midpoint(outgoing);
            let prev = Vertex::Midpoint(incoming);
            vec![match side {
                0 => control(outgoing, [v, next], [0., 1.]),
                1 => Side {
                    edge: Edge::Sector(patch.face, corner),
                    vertices: [next, center],
                    interval: [0., 1.],
                    mated: true,
                },
                2 => Side {
                    edge: Edge::Sector(patch.face, previous),
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

fn failure(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}
fn uv(side: usize, t: f64) -> Result<Point2, GeometryError> {
    let [u, v] = match side {
        0 => [t, 0.],
        1 => [1., t],
        2 => [1. - t, 1.],
        _ => [0., 1. - t],
    };
    Point2::try_new(u, v)
}
fn side_curve(
    surface: &NurbsSurface,
    side: usize,
    range: [f64; 2],
) -> Result<NurbsCurve, GeometryError> {
    let curve = match side {
        0 => surface.isocurve_u(0.),
        1 => surface.isocurve_v(1.),
        2 => surface.isocurve_u(1.),
        _ => surface.isocurve_v(0.),
    }?;
    let range = if side < 2 {
        range
    } else {
        [1. - range[1], 1. - range[0]]
    };
    let curve = curve.try_trimmed(range[0]..=range[1])?;
    let curve = if side < 2 { curve } else { curve.reversed()? };
    curve.try_reparameterized(0. ..=1.)
}

pub(super) fn assemble(
    source: &SubD,
    patches: Vec<SubDNurbsPatch>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let topology = SourceEdges::new(source)?;
    let mut vertices = Vec::new();
    let mut vertex_map = BTreeMap::new();
    let mut edges: Vec<BrepEdge> = Vec::new();
    let mut edge_map = BTreeMap::new();
    let mut faces = Vec::with_capacity(patches.len());
    for patch in patches {
        let mut trims = Vec::new();
        for side in 0..4 {
            for segment in topology.sides(source, &patch, side) {
                let parameters = [
                    uv(side, segment.interval[0])?,
                    uv(side, segment.interval[1])?,
                ];
                let mut ends = [0; 2];
                for (end, key) in segment.vertices.into_iter().enumerate() {
                    ends[end] = if let Some(&index) = vertex_map.get(&key) {
                        index
                    } else {
                        let point = if let Vertex::Control(i) = key {
                            source.surface_points()[i as usize]
                        } else {
                            patch
                                .surface
                                .evaluate(parameters[end].x(), parameters[end].y())?
                        };
                        let index = vertices.len();
                        vertices.push(BrepVertex::try_new(point, tolerance.absolute())?);
                        vertex_map.insert(key, index);
                        index
                    };
                }
                let edge = if let Some(&index) = edge_map.get(&segment.edge) {
                    index
                } else {
                    let index = edges.len();
                    edges.push(BrepEdge::try_new(
                        ends,
                        side_curve(&patch.surface, side, segment.interval)?,
                        tolerance.absolute(),
                    )?);
                    edge_map.insert(segment.edge, index);
                    index
                };
                let reversed = if edges[edge].vertices() == ends {
                    false
                } else if edges[edge].vertices() == [ends[1], ends[0]] {
                    true
                } else {
                    return Err(failure("inconsistent shared SubD patch endpoints"));
                };
                trims.push(BrepTrim::try_new(
                    ends,
                    Some(edge),
                    reversed,
                    NurbsCurve2::try_line(parameters[0], parameters[1])?,
                    if segment.mated {
                        BrepTrimType::Mated
                    } else {
                        BrepTrimType::Boundary
                    },
                    [
                        SurfaceIso::South,
                        SurfaceIso::East,
                        SurfaceIso::North,
                        SurfaceIso::West,
                    ][side],
                    [0., 0.],
                )?);
            }
        }
        faces.push(BrepFace::try_new(
            patch.surface,
            false,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?);
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}
