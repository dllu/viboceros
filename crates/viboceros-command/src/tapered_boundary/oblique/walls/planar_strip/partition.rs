//! Imprint covered sections while keeping the stock's existing wall joins.
use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::{BrepEdge, BrepLoopType, NurbsCurve2, SurfaceIso};

struct Section {
    curve: NurbsCurve,
    uv: Option<NurbsCurve2>,
    tolerance: f64,
    coverage: Coverage,
}

pub(super) fn apply(
    cut: &PlaneCut<'_>,
    stock: Brep,
    clipped: &Brep,
    tool: &Solid,
    rulings: &[[Ruling; 2]],
    traces: &[Option<Coverage>],
) -> Result<Brep, CommandError> {
    let tolerance = cut.tolerance;
    let mut splits = BTreeMap::new();
    let mut face_cuts = Vec::new();
    for (index, (face, ends)) in stock.faces().iter().zip(rulings).enumerate() {
        let section = trace(cut, face, clipped, index, tool, ends, traces)?;
        let mut covered_edges = [None; 2];
        for i in 0..2 {
            if section.coverage.reached[i] {
                let (edge, parameter) = ruling_edge(&stock, face, ends[i], tolerance)?;
                splits.entry(edge).or_insert(parameter);
                covered_edges[i] = Some(edge);
            }
        }
        if section.coverage.complete {
            let [Some(first), Some(second)] = covered_edges else {
                return Err(REJECTED);
            };
            face_cuts.push((index, [first, second], section));
        }
    }
    // Split each original ruling once, including both incident faces of
    // shared rulings. The new vertices are appended in source edge order.
    let splits = splits
        .into_iter()
        .map(|(edge, parameter)| (edge, vec![parameter]))
        .collect::<Vec<_>>();
    let first_vertex = stock.vertices().len();
    let stock = if splits.is_empty() {
        stock
    } else {
        stock.try_split_edges_at_parameters(&splits, tolerance)?
    };
    if face_cuts.is_empty() {
        if stock.faces().len() == 1 {
            return Ok(stock);
        }
        let order = (0..stock.faces().len()).rev().collect::<Vec<_>>();
        return Ok(stock.sub_brep(&order, tolerance)?);
    }

    let mut edges = stock.edges().to_vec();
    let mut faces = stock.faces().to_vec();
    let mut base_pieces = Vec::with_capacity(face_cuts.len());
    for (face, split_edges, section) in face_cuts {
        let vertex = |edge| {
            splits
                .binary_search_by_key(&edge, |(index, _)| *index)
                .map(|index| first_vertex + index)
                .map_err(|_| REJECTED)
        };
        let vertices = [vertex(split_edges[0])?, vertex(split_edges[1])?];
        let [far, base] = divide_face(&faces[face], vertices, &section, edges.len(), tolerance)?;
        edges.push(BrepEdge::try_new(
            vertices,
            section.curve,
            section.tolerance,
        )?);
        faces[face] = far;
        base_pieces.push(base);
    }
    // The retained strip traverses the original faces in reverse order and
    // carries the base pieces of divided faces at the end.
    faces.reverse();
    faces.extend(base_pieces.into_iter().rev());
    Ok(Brep::try_new(
        stock.vertices().to_vec(),
        edges,
        faces,
        tolerance,
    )?)
}

fn trace(
    cut: &PlaneCut<'_>,
    source: &BrepFace,
    clipped: &Brep,
    index: usize,
    tool: &Solid,
    rulings: &[Ruling; 2],
    traces: &[Option<Coverage>],
) -> Result<Section, CommandError> {
    let find = |brep: &Brep,
                face: &BrepFace,
                traces: &[Option<Coverage>]|
     -> Result<Option<Section>, CommandError> {
        for trim in face.loops().iter().flat_map(|boundary| boundary.trims()) {
            let index = trim.edge().ok_or(REJECTED)?;
            let edge = &brep.edges()[index];
            let curve = edge.curve();
            let cached = match traces.get(index) {
                Some(Some(coverage)) => Some(*coverage),
                None if cut.on_plane(curve) => None,
                _ => continue,
            };
            let domain = curve.domain();
            let ends = [
                curve.evaluate(*domain.start())?,
                curve.evaluate(*domain.end())?,
            ];
            let matches = |[a, b]: [Point3; 2]| {
                a.is_near(rulings[0].hit, cut.tolerance) && b.is_near(rulings[1].hit, cut.tolerance)
            };
            let reversed = if matches(ends) {
                false
            } else if matches([ends[1], ends[0]]) {
                true
            } else {
                continue;
            };
            let uv = if face.surface() != source.surface() {
                None
            } else if reversed != trim.is_reversed_3d() {
                Some(trim.curve().reversed()?)
            } else {
                Some(trim.curve().clone())
            };
            let mut coverage = match cached {
                Some(coverage) => coverage,
                None => cut.coverage(curve)?,
            };
            if reversed {
                coverage.reached.reverse();
            }
            return Ok(Some(Section {
                curve: if reversed {
                    curve.reversed()?
                } else {
                    curve.clone()
                },
                uv,
                tolerance: edge.tolerance(),
                coverage,
            }));
        }
        Ok(None)
    };
    // Reuse the section when the kernel preserved this face's slot and
    // carrier. An individual wall clip can recover reordered faces.
    if let Some(face) = clipped.faces().get(index)
        && face.surface() == source.surface()
        && let Some(curve) = find(clipped, face, traces)?
    {
        return Ok(curve);
    }
    let wall = Brep::try_surface_face(source.surface().clone(), cut.tolerance)?;
    let section = rulings.iter().flat_map(|ruling| [ruling.base, ruling.hit]);
    let wall = clip_inside(&wall, tool, section, cut.tolerance)?;
    find(&wall, &wall.faces()[0], &[])?.ok_or(REJECTED)
}

fn ruling_edge(
    stock: &Brep,
    face: &BrepFace,
    ruling: Ruling,
    tolerance: Tolerance,
) -> Result<(usize, f64), CommandError> {
    for trim in face.loops().iter().flat_map(|boundary| boundary.trims()) {
        let index = trim.edge().ok_or(REJECTED)?;
        let edge = &stock.edges()[index];
        if !edge.vertices().into_iter().any(|vertex| {
            stock.vertices()[vertex]
                .point()
                .is_near(ruling.base, tolerance)
        }) {
            continue;
        }
        let curve = edge.curve();
        let parameter = curve.closest_parameter(ruling.hit, tolerance)?;
        let domain = curve.domain();
        if parameter > *domain.start()
            && parameter < *domain.end()
            && curve.evaluate(parameter)?.is_near(ruling.hit, tolerance)
        {
            return Ok((index, parameter));
        }
    }
    Err(REJECTED)
}

fn divide_face(
    face: &BrepFace,
    vertices: [usize; 2],
    section: &Section,
    edge: usize,
    tolerance: Tolerance,
) -> Result<[BrepFace; 2], CommandError> {
    let [boundary] = face.loops() else {
        return Err(REJECTED);
    };
    let trims = boundary.trims();
    let position = |vertex| {
        trims
            .iter()
            .position(|trim| trim.vertices()[0] == vertex)
            .ok_or(REJECTED)
    };
    let [a, b] = [position(vertices[0])?, position(vertices[1])?];
    if a == b {
        return Err(REJECTED);
    }
    let surface = face.surface();
    let ends = [
        trims[a].curve().start_point()?,
        trims[b].curve().start_point()?,
    ];
    let uv = if let Some(uv) = &section.uv
        && uv_ends_match(uv, ends, tolerance)?
    {
        uv.clone()
    } else {
        surface.try_pullback_curve_certified_with_endpoints(&section.curve, ends, tolerance)?
    };
    let piece = |start: usize, end: usize, reversed: bool, uv| {
        let mut path = (start..end)
            .map(|index| trims[index % trims.len()].clone())
            .collect::<Vec<_>>();
        let vertices = if reversed {
            [vertices[1], vertices[0]]
        } else {
            vertices
        };
        path.push(BrepTrim::try_new(
            vertices,
            Some(edge),
            reversed,
            uv,
            BrepTrimType::Mated,
            SurfaceIso::NotIso,
            [0.; 2],
        )?);
        BrepFace::try_new(
            surface.clone(),
            face.is_reversed(),
            vec![BrepLoop::try_new(BrepLoopType::Outer, path)?],
        )
    };
    let first = piece(
        a,
        if b > a { b } else { b + trims.len() },
        true,
        uv.reversed()?,
    )?;
    let second = piece(b, if a > b { a } else { a + trims.len() }, false, uv)?;
    let first_faces_far = trims[a].curve().end_point()?.x() > ends[0].x();
    Ok(if first_faces_far {
        [first, second]
    } else {
        [second, first]
    })
}

fn uv_ends_match(
    uv: &NurbsCurve2,
    ends: [Point2; 2],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    Ok([uv.start_point()?, uv.end_point()?]
        .into_iter()
        .zip(ends)
        .all(|(a, b)| {
            (a.x() - b.x()).abs() <= tolerance.absolute()
                && (a.y() - b.y()).abs() <= tolerance.absolute()
        }))
}
