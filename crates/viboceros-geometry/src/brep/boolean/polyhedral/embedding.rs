//! Input faces may meet only along their shared topological edges/vertices.
use super::*;

pub(super) fn certify(
    brep: &Brep,
    polygons: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    certify_vertex_links(brep, budget)?;
    let bounds = polygons
        .iter()
        .map(|p| {
            std::array::from_fn::<_, 3, _>(|i| {
                [
                    p.ring.iter().map(|v| v[i].clone()).min().unwrap(),
                    p.ring.iter().map(|v| v[i].clone()).max().unwrap(),
                ]
            })
        })
        .collect::<Vec<_>>();
    for i in 0..polygons.len() {
        for j in i + 1..polygons.len() {
            budget.spend(1)?;
            let (a, b) = (&polygons[i], &polygons[j]);
            if std::ptr::eq(a.source, b.source)
                || (0..3)
                    .any(|k| bounds[i][k][1] < bounds[j][k][0] || bounds[j][k][1] < bounds[i][k][0])
            {
                continue;
            }
            let direction = cross(&a.normal, &b.normal);
            check_point(&direction)?;
            if zero(&direction) {
                if !a.plane_side(&b.ring[0]).is_zero() {
                    continue;
                }
                let mut overlap = Some(a.ring.clone());
                for k in 0..b.ring.len() {
                    let Some(ring) = overlap else { break };
                    let normal =
                        cross(&sub(&b.ring[(k + 1) % b.ring.len()], &b.ring[k]), &b.normal);
                    overlap = split_plane(&ring, &b.ring[k], &normal, budget)?.0;
                }
                if overlap.is_some() {
                    return Err(unsupported("overlapping coplanar input faces"));
                }
                for k in 0..a.ring.len() {
                    for l in 0..b.ring.len() {
                        let points = input::segment_contacts(
                            &a.ring[k],
                            &a.ring[(k + 1) % a.ring.len()],
                            &b.ring[l],
                            &b.ring[(l + 1) % b.ring.len()],
                            &a.normal,
                            budget,
                        )?;
                        for p in &points {
                            allowed_contact(brep, a.source, b.source, p, budget)?;
                        }
                        if points.len() == 2 {
                            allowed_contact(
                                brep,
                                a.source,
                                b.source,
                                &mean(&points, budget)?,
                                budget,
                            )?;
                        }
                    }
                }
            } else {
                let axis = direction.iter().position(|v| !v.is_zero()).unwrap();
                let left = plane_section(a, b, axis, budget)?;
                let right = plane_section(b, a, axis, budget)?;
                if let (Some([a0, a1]), Some([b0, b1])) = (left, right) {
                    let low = if a0[axis] >= b0[axis] { a0 } else { b0 };
                    let high = if a1[axis] <= b1[axis] { a1 } else { b1 };
                    if low[axis] <= high[axis] {
                        allowed_contact(brep, a.source, b.source, &low, budget)?;
                        allowed_contact(brep, a.source, b.source, &high, budget)?;
                        if low != high {
                            allowed_contact(
                                brep,
                                a.source,
                                b.source,
                                &mean(&[low, high], budget)?,
                                budget,
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn certify_vertex_links(brep: &Brep, budget: &mut Budget) -> Result<(), GeometryError> {
    let mut users = vec![Vec::new(); brep.edges.len()];
    for (index, face) in brep.faces.iter().enumerate() {
        for trim in face.loops.iter().flat_map(|l| &l.trims) {
            budget.spend(1)?;
            users[trim.edge.ok_or_else(|| unsupported("singular trims"))?].push(index);
        }
    }
    let mut links = vec![BTreeMap::<usize, Vec<usize>>::new(); brep.vertices.len()];
    for (edge, faces) in brep.edges.iter().zip(users) {
        let [a, b] = faces.as_slice() else {
            return Err(unsupported("closed manifold edges are required"));
        };
        for vertex in edge.vertices {
            budget.spend(1)?;
            links[vertex].entry(*a).or_default().push(*b);
            links[vertex].entry(*b).or_default().push(*a);
        }
    }
    for link in links {
        if link.is_empty() {
            continue;
        }
        if link.values().any(|neighbors| neighbors.len() != 2) {
            return Err(unsupported("nonmanifold vertex link"));
        }
        let mut seen = BTreeSet::new();
        let mut stack = vec![*link.keys().next().unwrap()];
        while let Some(face) = stack.pop() {
            budget.spend(1)?;
            if seen.insert(face) {
                stack.extend(&link[&face]);
            }
        }
        if seen.len() != link.len() {
            return Err(unsupported("disconnected vertex link"));
        }
    }
    Ok(())
}

pub(super) fn plane_section(
    polygon: &Polygon<'_>,
    plane: &Polygon<'_>,
    axis: usize,
    budget: &mut Budget,
) -> Result<Option<[ExactPoint; 2]>, GeometryError> {
    let mut points = BTreeSet::new();
    for i in 0..polygon.ring.len() {
        budget.spend(1)?;
        let (a, b) = (
            &polygon.ring[i],
            &polygon.ring[(i + 1) % polygon.ring.len()],
        );
        let (sa, sb) = (plane.plane_side(a), plane.plane_side(b));
        if sa.is_zero() {
            points.insert(a.clone());
        }
        if (sa.is_positive() && sb.is_negative()) || (sa.is_negative() && sb.is_positive()) {
            let t = &sa / (&sa - &sb);
            let p = std::array::from_fn(|j| &a[j] + &t * (&b[j] - &a[j]));
            check_point(&p)?;
            points.insert(p);
        }
    }
    if points.is_empty() {
        return Ok(None);
    }
    Ok(Some([
        points.iter().min_by_key(|p| &p[axis]).unwrap().clone(),
        points.iter().max_by_key(|p| &p[axis]).unwrap().clone(),
    ]))
}

fn allowed_contact(
    brep: &Brep,
    a: &BrepFace,
    b: &BrepFace,
    p: &ExactPoint,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let edges = |face: &BrepFace| {
        face.loops
            .iter()
            .flat_map(|l| &l.trims)
            .filter_map(|t| t.edge)
            .collect::<BTreeSet<_>>()
    };
    let (ae, be) = (edges(a), edges(b));
    for edge in ae.intersection(&be) {
        budget.spend(1)?;
        let [start, end] = brep.edges[*edge]
            .vertices
            .map(|i| point(brep.vertices[i].point));
        if input::on_segment(p, &start, &end) {
            return Ok(());
        }
    }
    let vertices = |face: &BrepFace| {
        face.loops
            .iter()
            .flat_map(|l| &l.trims)
            .flat_map(|t| t.vertices)
            .collect::<BTreeSet<_>>()
    };
    let (av, bv) = (vertices(a), vertices(b));
    for vertex in av.intersection(&bv) {
        budget.spend(1)?;
        if *p == point(brep.vertices[*vertex].point) {
            return Ok(());
        }
    }
    Err(unsupported("input faces intersect without shared topology"))
}
