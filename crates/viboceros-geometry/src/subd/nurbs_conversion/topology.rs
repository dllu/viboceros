//! Sew by source component identity, including half-edges beside n-gons.
use super::*;
use crate::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve, NurbsCurve2, Point2, SurfaceIso, Tolerance,
};
use std::collections::BTreeMap;

mod deviation;
mod perimeter;
mod source;
use source::{Edge, Side, SourceEdges, Vertex};

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
    assemble_packed(
        source,
        patches
            .into_iter()
            .map(|p| SubDNurbsPackedPatch {
                grid_size: [1, 1],
                tiles: vec![SubDNurbsTile {
                    face: p.face,
                    corner: p.corner,
                    cell: [0, 0],
                    quarter_turns: 0,
                }],
                surface: p.surface,
            })
            .collect(),
        tolerance,
    )
}

pub(super) fn assemble_packed(
    source: &SubD,
    packs: Vec<SubDNurbsPackedPatch>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let perimeters = perimeter::build(source, &packs)?;
    let mut owners = BTreeMap::<perimeter::Key, Vec<usize>>::new();
    for (face, segments) in perimeters.iter().enumerate() {
        for s in segments {
            owners.entry(s.key.clone()).or_default().push(face);
        }
    }
    let mut vertices = Vec::new();
    let mut vertex_map = BTreeMap::new();
    let mut edges: Vec<BrepEdge> = Vec::new();
    let mut edge_map = BTreeMap::<perimeter::Key, usize>::new();
    let mut faces = Vec::with_capacity(packs.len());
    let soft = source.edge_sharpness().iter().flatten().any(|&s| s > 0.);
    for (face_index, (pack, segments)) in packs.into_iter().zip(perimeters).enumerate() {
        let mut trims = Vec::with_capacity(segments.len());
        for segment in segments {
            let parameters = [
                uv(segment.side, segment.interval[0])?,
                uv(segment.side, segment.interval[1])?,
            ];
            let mut ends = [0; 2];
            for (end, key) in segment.vertices.into_iter().enumerate() {
                ends[end] = if let Some(&i) = vertex_map.get(&key) {
                    i
                } else {
                    let point = if let Vertex::Control(i) = key {
                        source.surface_points()[i as usize]
                    } else {
                        pack.surface
                            .evaluate(parameters[end].x(), parameters[end].y())?
                    };
                    let index = vertices.len();
                    vertices.push(BrepVertex::try_new(point, tolerance.absolute())?);
                    vertex_map.insert(key, index);
                    index
                };
            }
            let ordered = if segment.reversed {
                [ends[1], ends[0]]
            } else {
                ends
            };
            let edge = if let Some(&index) = edge_map.get(&segment.key) {
                if soft {
                    let mut curve = side_curve(&pack.surface, segment.side, segment.interval)?;
                    if segment.reversed {
                        curve = curve.reversed()?;
                    }
                    let old = &edges[index];
                    let allowed = deviation::bound(old.curve(), &curve, tolerance.absolute())?
                        .max(old.tolerance());
                    edges[index] = BrepEdge::try_new(old.vertices(), old.curve().clone(), allowed)?;
                }
                index
            } else {
                let mut curve = side_curve(&pack.surface, segment.side, segment.interval)?;
                if segment.reversed {
                    curve = curve.reversed()?;
                }
                let index = edges.len();
                edges.push(BrepEdge::try_new(ordered, curve, tolerance.absolute())?);
                edge_map.insert(segment.key.clone(), index);
                index
            };
            if edges[edge].vertices() != ordered {
                return Err(failure("Inconsistent shared SubD packed endpoints"));
            }
            let uses = &owners[&segment.key];
            let trim_type = if segment.mated {
                if uses.len() != 2 {
                    return Err(failure("Unpaired SubD packed perimeter"));
                }
                if uses[0] == face_index && uses[1] == face_index {
                    BrepTrimType::Seam
                } else {
                    BrepTrimType::Mated
                }
            } else {
                if uses.len() != 1 {
                    return Err(failure("Ambiguous SubD packed boundary"));
                }
                BrepTrimType::Boundary
            };
            trims.push(BrepTrim::try_new(
                ends,
                Some(edge),
                segment.reversed,
                NurbsCurve2::try_line(parameters[0], parameters[1])?,
                trim_type,
                [
                    SurfaceIso::South,
                    SurfaceIso::East,
                    SurfaceIso::North,
                    SurfaceIso::West,
                ][segment.side],
                [0., 0.],
            )?);
        }
        faces.push(BrepFace::try_new(
            pack.surface,
            false,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?);
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}
