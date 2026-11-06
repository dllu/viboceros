//! Independent-contact nesting retained for PlanarMesh.
use super::*;
pub(super) fn groups(
    boundaries: &[PlanarSurfaceBoundary],
    tolerance: Tolerance,
) -> Result<Vec<Vec<usize>>, CommandError> {
    // A strict containment forest applies the same even-odd region rule
    // as planar CAD boundaries: children cut holes, grandchildren become
    // independent islands, and disjoint or partially overlapping loops
    // remain independent faces. The smallest containing loop is the
    // immediate parent when several levels are nested.
    let mut parents: Vec<Option<usize>> = vec![None; boundaries.len()];
    let mut indices_by_area = (0..boundaries.len()).collect::<Vec<_>>();
    indices_by_area.sort_by(|left, right| {
        boundaries[*left]
            .area
            .total_cmp(&boundaries[*right].area)
            .then(left.cmp(right))
    });
    for child_index in 0..boundaries.len() {
        for &outer_index in &indices_by_area {
            if outer_index == child_index
                || boundaries[outer_index].area <= boundaries[child_index].area
                || !bounds_contain_point(
                    boundaries[outer_index].bounds,
                    boundaries[child_index].probe,
                    tolerance,
                )
            {
                continue;
            }
            if Brep::try_planar_face_with_holes(
                &boundaries[outer_index].curve,
                std::slice::from_ref(&boundaries[child_index].curve),
                tolerance,
            )
            .is_ok()
                && !curves_contact(
                    &boundaries[outer_index].curve,
                    &boundaries[child_index].curve,
                    tolerance,
                )?
            {
                parents[child_index] = Some(outer_index);
                break;
            }
        }
    }

    // Intersecting siblings cannot both be trim holes. Rhino treats
    // partially overlapping input curves as independent surfaces, so
    // detach both while leaving unrelated siblings assigned as holes.
    let mut detached = vec![false; boundaries.len()];
    for parent_index in 0..boundaries.len() {
        let children = parents
            .iter()
            .enumerate()
            .filter_map(|(index, parent)| (*parent == Some(parent_index)).then_some(index))
            .collect::<Vec<_>>();
        for first_position in 0..children.len() {
            for second_position in (first_position + 1)..children.len() {
                let first = children[first_position];
                let second = children[second_position];
                if !bounds_overlap(
                    boundaries[first].bounds,
                    boundaries[second].bounds,
                    tolerance,
                ) {
                    continue;
                }
                let holes = [
                    boundaries[first].curve.clone(),
                    boundaries[second].curve.clone(),
                ];
                if Brep::try_planar_face_with_holes(
                    &boundaries[parent_index].curve,
                    &holes,
                    tolerance,
                )
                .is_err()
                {
                    detached[first] = true;
                    detached[second] = true;
                }
            }
        }
    }
    for (parent, detached) in parents.iter_mut().zip(detached) {
        if detached {
            *parent = None;
        }
    }
    let depths = (0..boundaries.len())
        .map(|index| {
            let mut depth = 0_usize;
            let mut ancestor = parents[index];
            while let Some(parent) = ancestor {
                depth += 1;
                debug_assert!(depth < boundaries.len());
                ancestor = parents[parent];
            }
            depth
        })
        .collect::<Vec<_>>();

    Ok((0..boundaries.len())
        .filter(|&i| depths[i] % 2 == 0)
        .map(|outer| {
            std::iter::once(outer)
                .chain(
                    parents
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| (*p == Some(outer)).then_some(i)),
                )
                .collect()
        })
        .collect())
}
