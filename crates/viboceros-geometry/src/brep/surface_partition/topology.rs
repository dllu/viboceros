//! Sew rectangle sides after inserting all incident corner coordinates.
use super::*;
use std::collections::BTreeSet;

struct Edge {
    endpoints: [usize; 2],
    curve: NurbsCurve,
    singular: bool,
    faces: Vec<usize>,
}
struct Trim {
    edge: usize,
    side: usize,
    points: [[Real; 2]; 2],
    endpoints: [usize; 2],
}

pub(super) fn build(
    surface: &NurbsSurface,
    cells: Vec<NurbsSurface>,
    tolerance: Tolerance,
    budget: &mut usize,
) -> Result<Brep, GeometryError> {
    let closed = [surface.is_closed_u()?, surface.is_closed_v()?];
    let mut coordinates: [Vec<Real>; 2] = [Vec::new(), Vec::new()];
    for cell in &cells {
        for (values, domain) in coordinates
            .iter_mut()
            .zip([cell.domain_u(), cell.domain_v()])
        {
            values.extend([*domain.start(), *domain.end()].map(coordinate));
        }
    }
    for values in &mut coordinates {
        values.sort_by(Real::total_cmp);
        values.dedup();
    }
    let lookup = |axis: usize, t: Real| {
        coordinates[axis]
            .binary_search_by(|x| x.total_cmp(&coordinate(t)))
            .unwrap()
    };
    let canonical = |axis: usize, i: usize| {
        if closed[axis] && i + 1 == coordinates[axis].len() {
            0
        } else {
            i
        }
    };
    let ranges = cells
        .iter()
        .map(|cell| {
            [cell.domain_u(), cell.domain_v()]
                .into_iter()
                .enumerate()
                .map(|(a, d)| [lookup(a, *d.start()), lookup(a, *d.end())])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut lines = BTreeMap::<(usize, usize), BTreeSet<usize>>::new();
    for r in &ranges {
        for i in r[0] {
            for j in r[1] {
                lines.entry((0, canonical(1, j))).or_default().insert(i);
                lines.entry((1, canonical(0, i))).or_default().insert(j);
            }
        }
    }
    let mut points = Vec::<[usize; 2]>::new();
    let mut point_map = BTreeMap::new();
    let mut parent = Vec::new();
    let mut edges = Vec::<Edge>::new();
    let mut edge_map = BTreeMap::new();
    let mut isocurves = BTreeMap::<(usize, usize), NurbsCurve>::new();
    let mut rings = Vec::new();
    for (face, r) in ranges.iter().enumerate() {
        let mut trims = Vec::new();
        for (side, (axis, fixed, interval)) in [
            (0, r[1][0], r[0]),
            (1, r[0][1], r[1]),
            (0, r[1][1], r[0]),
            (1, r[0][0], r[1]),
        ]
        .into_iter()
        .enumerate()
        {
            let constant = canonical(1 - axis, fixed);
            let values = lines[&(axis, constant)]
                .range(interval[0]..=interval[1])
                .copied()
                .collect::<Vec<_>>();
            charge(budget, values.len())?;
            let mut parts = Vec::new();
            for pair in values.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                let native = |t| if axis == 0 { [t, fixed] } else { [fixed, t] };
                let uv = [native(a), native(b)];
                let mut endpoints = [0; 2];
                for (slot, p) in endpoints.iter_mut().zip(uv) {
                    let p = [canonical(0, p[0]), canonical(1, p[1])];
                    *slot = *point_map.entry(p).or_insert_with(|| {
                        let id = points.len();
                        points.push(p);
                        parent.push(id);
                        id
                    });
                }
                let key = (axis, constant, a, b);
                let edge = if let Some(&edge) = edge_map.get(&key) {
                    edge
                } else {
                    let line = (axis, constant);
                    if let std::collections::btree_map::Entry::Vacant(entry) = isocurves.entry(line)
                    {
                        charge(budget, surface.control_points().len())?;
                        entry.insert(if axis == 0 {
                            surface.isocurve_u(coordinates[1][constant])?
                        } else {
                            surface.isocurve_v(coordinates[0][constant])?
                        });
                    }
                    let curve = &isocurves[&line];
                    charge(budget, curve.control_points().len().saturating_mul(4))?;
                    let curve = curve.try_trimmed(coordinates[axis][a]..=coordinates[axis][b])?;
                    let singular = curve
                        .control_points()
                        .iter()
                        .all(|c| c.point() == curve.control_points()[0].point());
                    if singular {
                        super::super::surface_grid::unite(&mut parent, endpoints[0], endpoints[1]);
                    }
                    let edge = edges.len();
                    edges.push(Edge {
                        endpoints,
                        curve,
                        singular,
                        faces: Vec::new(),
                    });
                    edge_map.insert(key, edge);
                    edge
                };
                edges[edge].faces.push(face);
                let mut uv = uv.map(|p| [coordinates[0][p[0]], coordinates[1][p[1]]]);
                if side >= 2 {
                    endpoints.reverse();
                    uv.reverse();
                }
                parts.push(Trim {
                    edge,
                    side,
                    points: uv,
                    endpoints,
                });
            }
            if side >= 2 {
                parts.reverse();
            }
            trims.extend(parts);
        }
        rings.push(trims);
    }
    let mut vertices = Vec::new();
    let mut roots = BTreeMap::new();
    let mut indices = Vec::new();
    for i in 0..points.len() {
        let root = super::super::surface_grid::root(&parent, i);
        let index = if let Some(&index) = roots.get(&root) {
            index
        } else {
            let p = points[root];
            let index = vertices.len();
            vertices.push(BrepVertex::try_new(
                surface.evaluate(coordinates[0][p[0]], coordinates[1][p[1]])?,
                tolerance.absolute(),
            )?);
            roots.insert(root, index);
            index
        };
        indices.push(index);
    }
    let mut output_edges = Vec::new();
    let mut edge_indices = Vec::new();
    for edge in &edges {
        edge_indices.push(if edge.singular {
            None
        } else {
            let index = output_edges.len();
            output_edges.push(BrepEdge::try_new(
                edge.endpoints.map(|p| indices[p]),
                edge.curve.clone(),
                tolerance.absolute(),
            )?);
            Some(index)
        });
    }
    let mut faces = Vec::new();
    for (cell, ring) in cells.into_iter().zip(rings) {
        let mut trims = Vec::new();
        for trim in ring {
            let edge = &edges[trim.edge];
            let kind = if edge.singular {
                BrepTrimType::Singular
            } else if edge.faces.len() == 1 {
                BrepTrimType::Boundary
            } else if edge.faces.len() == 2 && edge.faces[0] == edge.faces[1] {
                BrepTrimType::Seam
            } else {
                BrepTrimType::Mated
            };
            trims.push(BrepTrim::try_new(
                trim.endpoints.map(|p| indices[p]),
                edge_indices[trim.edge],
                !edge.singular && trim.side >= 2,
                NurbsCurve2::try_line(
                    Point2::try_from(trim.points[0])?,
                    Point2::try_from(trim.points[1])?,
                )?,
                kind,
                [
                    SurfaceIso::South,
                    SurfaceIso::East,
                    SurfaceIso::North,
                    SurfaceIso::West,
                ][trim.side],
                [0.; 2],
            )?);
        }
        faces.push(BrepFace::try_new(
            cell,
            false,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?);
    }
    Brep::try_new(vertices, output_edges, faces, tolerance)
}

fn coordinate(t: Real) -> Real {
    // Numeric UV identity is independent of the sign bit on zero.
    if t == 0. { 0. } else { t }
}
