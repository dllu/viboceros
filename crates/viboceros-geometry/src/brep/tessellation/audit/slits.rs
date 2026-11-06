//! Exact opposite line uses in one loop cancel as zero-area slits.
use super::*;

pub(super) fn mark(brep: &Brep, uses: &[(usize, usize)], matched: &mut [bool]) {
    for face in &brep.faces {
        for boundary in &face.loops {
            let mut pairs = BTreeMap::new();
            for trim in &boundary.trims {
                let Some(edge) = trim.edge.filter(|&e| uses[e].0 == 1) else {
                    continue;
                };
                let spatial = &brep.edges[edge].curve;
                let parameter = &trim.curve;
                if spatial.degree() != 1
                    || spatial.control_points().len() != 2
                    || parameter.degree() != 1
                    || parameter.control_points().len() != 2
                {
                    continue;
                }
                if spatial.control_points()[0].weight().is_sign_positive()
                    != spatial.control_points()[1].weight().is_sign_positive()
                    || parameter.control_points()[0].weight().is_sign_positive()
                        != parameter.control_points()[1].weight().is_sign_positive()
                {
                    continue;
                }
                let mut world = spatial
                    .control_points()
                    .iter()
                    .map(|p| key(p.point()))
                    .collect::<Vec<_>>();
                if trim.reversed_3d {
                    world.reverse();
                }
                let uv = parameter
                    .control_points()
                    .iter()
                    .map(|p| {
                        p.point()
                            .to_array()
                            .map(|x| if x == 0. { 0 } else { x.to_bits() })
                    })
                    .collect::<Vec<_>>();
                let mut ends = [(uv[0], world[0]), (uv[1], world[1])];
                if ends[0] == ends[1] {
                    continue;
                }
                let reversed = ends[0] > ends[1];
                if reversed {
                    ends.swap(0, 1);
                }
                pairs
                    .entry(ends)
                    .or_insert_with(Vec::new)
                    .push((edge, reversed));
            }
            for uses in pairs.values() {
                // A complete pair of identical straight loci, opposite in the
                // same UV loop, contributes zero to the oriented region. Do
                // not infer cancellation from shared endpoints alone, from
                // partial overlaps, or from edges on different hole loops.
                if let [(a, forward), (b, backward)] = uses.as_slice()
                    && forward != backward
                {
                    matched[*a] = true;
                    matched[*b] = true;
                }
            }
        }
    }
}
