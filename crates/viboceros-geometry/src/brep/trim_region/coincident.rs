//! Repeated polygon walks contribute one geometric hole to triangulation.
use std::{collections::BTreeSet, ops::Range};
pub(in crate::brep) fn unique_loops(
    ranges: &[Range<usize>],
    representatives: &[usize],
) -> Vec<Range<usize>> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(ranges.len());
    for (i, range) in ranges.iter().enumerate() {
        if i == 0 {
            result.push(range.clone());
            continue;
        }
        let key = canonical_walk(&representatives[range.clone()], true);
        if seen.insert(key) {
            result.push(range.clone());
        }
    }
    result
}
pub(in crate::brep) fn canonical_walk(ids: &[usize], closed: bool) -> Vec<usize> {
    let mut walk = ids.to_vec();
    walk.dedup();
    if closed && walk.len() > 1 && walk.first() == walk.last() {
        walk.pop();
    }
    let forward = if closed {
        rotation(&walk)
    } else {
        walk.clone()
    };
    walk.reverse();
    let reverse = if closed { rotation(&walk) } else { walk };
    forward.min(reverse)
}
// Booth's algorithm keeps repeated seam vertices linear in the walk length.
fn rotation(walk: &[usize]) -> Vec<usize> {
    let n = walk.len();
    if n < 2 {
        return walk.to_vec();
    }
    let (mut i, mut j, mut k) = (0, 1, 0);
    while i < n && j < n && k < n {
        match walk[(i + k) % n].cmp(&walk[(j + k) % n]) {
            std::cmp::Ordering::Equal => {
                k += 1;
                continue;
            }
            std::cmp::Ordering::Greater => {
                i += k + 1;
                if i == j {
                    i += 1;
                }
            }
            std::cmp::Ordering::Less => {
                j += k + 1;
                if i == j {
                    j += 1;
                }
            }
        }
        k = 0;
    }
    let start = i.min(j);
    (0..n).map(|k| walk[(start + k) % n]).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Brep, NurbsCurve, Point3, Tolerance};
    #[test]
    fn only_identical_closed_walks_are_coalesced() {
        let ranges = [0..4, 4..8, 8..12];
        assert_eq!(
            unique_loops(&ranges, &[0, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 7]),
            vec![0..4, 4..8]
        );
        assert_eq!(
            unique_loops(&ranges, &[0, 1, 2, 3, 4, 5, 6, 7, 4, 5, 8, 9]),
            ranges
        );
        for walk in [
            vec![4, 5, 4, 7, 4, 6],
            vec![4, 4, 4, 4],
            vec![7, 4, 5, 6, 4, 5],
        ] {
            let key = rotation(&walk);
            for shift in 0..walk.len() {
                let rotated = (0..walk.len())
                    .map(|i| walk[(i + shift) % walk.len()])
                    .collect::<Vec<_>>();
                assert_eq!(rotation(&rotated), key);
            }
        }
    }
    #[test]
    fn repeated_polygon_holes_retain_model_loops_and_mesh_the_geometric_region() {
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
        let hole = rectangle(2., 2., 6., 6.);
        let reversed = hole.reversed().unwrap();
        let brep = Brep::try_planar_face_with_holes(
            &outer,
            &[hole.clone(), reversed.clone()],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(brep.faces()[0].loops().len(), 3);
        assert_eq!(brep.edges()[1].curve(), &hole);
        assert_eq!(brep.edges()[2].curve(), &reversed);
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 68.).abs() < 1e-10);
        let mesh = brep.tessellate(8, Tolerance::DEFAULT).unwrap();
        assert!((mesh.area().unwrap() - 84.).abs() < 1e-10);
    }
}
