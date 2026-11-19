//! Refine long chords inside a trimmed face without changing shared edges.
use super::*;

fn edge(a: u32, b: u32) -> [u32; 2] {
    [a.min(b), a.max(b)]
}

fn distance_to_chord(point: Point3, a: Point3, b: Point3) -> Result<Real, GeometryError> {
    let axis = a.vector_to(b)?;
    let length_squared = axis.dot(axis)?;
    if length_squared == 0. {
        return point.distance_to(a);
    }
    let position = (a.vector_to(point)?.dot(axis)? / length_squared).clamp(0., 1.);
    point.distance_to(a.translated(axis.scaled(position)?)?)
}

pub(super) fn refine_interior(
    surface: &NurbsSurface,
    parameters: &mut Vec<Point2>,
    points: &mut Vec<Point3>,
    triangles: &mut Vec<[u32; 3]>,
    chord_height: Real,
    limit: usize,
) -> Result<(), GeometryError> {
    while parameters.len() < limit {
        let mut edges = triangles
            .iter()
            .flat_map(|&[a, b, c]| [edge(a, b), edge(b, c), edge(c, a)])
            .collect::<Vec<_>>();
        edges.sort_unstable();
        let mut proposals = Vec::new();
        for incidence in edges.chunk_by(|a, b| a == b) {
            // A one-sided edge is an installed face boundary. Inserting a
            // vertex there would make a T-junction on the mating face.
            if incidence.len() != 2 {
                continue;
            }
            let key = incidence[0];
            let [a, b] = key.map(|i| i as usize);
            let [ua, ub] = [parameters[a], parameters[b]];
            let uv = Point2::try_new(ua.x().midpoint(ub.x()), ua.y().midpoint(ub.y()))?;
            if uv == ua || uv == ub {
                continue;
            }
            let point = surface.evaluate(uv.x(), uv.y())?;
            let error = distance_to_chord(point, points[a], points[b])?;
            if error > chord_height {
                proposals.push((error, key, uv, point));
            }
        }
        if proposals.is_empty() {
            break;
        }
        proposals.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut cuts = BTreeMap::new();
        for (_, key, uv, point) in proposals.into_iter().take(limit - parameters.len()) {
            cuts.insert(key, parameters.len() as u32);
            parameters.push(uv);
            points.push(point);
        }
        let original = std::mem::take(triangles);
        triangles.reserve(original.len() + cuts.len() * 2);
        for triangle in original {
            split(triangle, &cuts, triangles);
        }
    }
    Ok(())
}

fn split(triangle: [u32; 3], cuts: &BTreeMap<[u32; 2], u32>, output: &mut Vec<[u32; 3]>) {
    let [a, b, c] = triangle;
    let mut mids = [edge(a, b), edge(b, c), edge(c, a)].map(|k| cuts.get(&k).copied());
    if let [Some(ab), Some(bc), Some(ca)] = mids {
        output.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        return;
    }
    let mut t = triangle;
    // Rotate to place the only split first, or the two splits consecutively.
    while mids[0].is_none() && (mids[1].is_some() || mids[2].is_some())
        || mids[0].is_some() && mids[1].is_none() && mids[2].is_some()
    {
        t.rotate_left(1);
        mids.rotate_left(1);
    }
    let [a, b, c] = t;
    match mids {
        [Some(ab), Some(bc), None] => output.extend([[a, ab, c], [ab, bc, c], [ab, b, bc]]),
        [Some(ab), None, None] => output.extend([[a, ab, c], [ab, b, c]]),
        _ => output.push(triangle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interior_chord_refines_both_incident_triangles_without_splitting_trims() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 8., 0., 3.).unwrap();
        let [start, end] = [*surface.domain_u().start(), *surface.domain_u().end()];
        let quarter = start + (end - start) * 0.25;
        let mut uv = [[start, 0.], [quarter, 0.], [quarter, 3.], [start, 3.]]
            .map(|[u, v]| Point2::try_new(u, v).unwrap())
            .to_vec();
        let mut points = uv
            .iter()
            .map(|p| surface.evaluate(p.x(), p.y()).unwrap())
            .collect();
        let mut triangles = vec![[0, 1, 2], [0, 2, 3]];
        refine_interior(&surface, &mut uv, &mut points, &mut triangles, 0.1, 5).unwrap();
        assert_eq!(uv.len(), 5);
        assert_eq!(triangles.len(), 4);
        assert!(triangles.iter().all(|triangle| triangle.contains(&4)));
        let mut uses = BTreeMap::new();
        for [a, b, c] in triangles {
            for side in [edge(a, b), edge(b, c), edge(c, a)] {
                *uses.entry(side).or_insert(0) += 1;
            }
        }
        assert_eq!(
            uses.iter()
                .filter(|&(_, &n)| n == 1)
                .map(|(&k, _)| k)
                .collect::<Vec<_>>(),
            [[0, 1], [0, 3], [1, 2], [2, 3]]
        );
    }
}
