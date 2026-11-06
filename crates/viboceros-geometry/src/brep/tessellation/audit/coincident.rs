//! A complete identical linear boundary can witness several naked edge records.
use super::*;
pub(super) fn mark(brep: &Brep, naked_by_face: &[Vec<usize>], matched: &mut [bool]) {
    let mut points = BTreeMap::new();
    for edges in naked_by_face {
        let mut groups = BTreeMap::<(bool, Vec<usize>), Vec<usize>>::new();
        for &e in edges {
            let c = &brep.edges[e].curve;
            let sign = c.control_points()[0].weight().is_sign_positive();
            if c.degree() != 1
                || c.control_points()
                    .iter()
                    .any(|p| p.weight().is_sign_positive() != sign)
                || c.knots()[1..c.knots().len() - 1]
                    .windows(2)
                    .any(|p| p[0] >= p[1])
            {
                continue;
            }
            let ids = c
                .control_points()
                .iter()
                .map(|p| {
                    let next = points.len();
                    *points.entry(key(p.point())).or_insert(next)
                })
                .collect::<Vec<_>>();
            let closed = ids.first() == ids.last();
            let walk = trim_region::canonical_walk(&ids, closed);
            groups.entry((closed, walk)).or_default().push(e);
        }
        for group in groups.values() {
            if group.iter().any(|&e| matched[e]) {
                for &e in group {
                    matched[e] = true;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_shared_segment_does_not_witness_an_entire_missing_hole() {
        let rectangle = |a, b, c, d| {
            NurbsCurve::try_new(
                1,
                [[a, b, 0.], [c, b, 0.], [c, d, 0.], [a, d, 0.], [a, b, 0.]]
                    .into_iter()
                    .map(|p| Point3::try_from(p).unwrap())
                    .collect(),
                vec![0., 0., 1., 2., 3., 4., 4.],
            )
            .unwrap()
        };
        let outer = rectangle(0., 0., 10., 10.);
        let a = rectangle(1., 1., 4., 4.);
        let b = rectangle(4., 1., 7., 4.);
        let tol = Tolerance::DEFAULT;
        let incomplete = Brep::try_planar_face_with_holes(&outer, std::slice::from_ref(&a), tol)
            .unwrap()
            .tessellate(8, tol)
            .unwrap();
        let complete = Brep::try_planar_face_with_holes(&outer, &[a, b], tol).unwrap();
        assert!(
            !complete
                .mesh_boundary_conforms(&incomplete, &vec![0; incomplete.faces().len()], 8, tol)
                .unwrap()
        );
    }
}
