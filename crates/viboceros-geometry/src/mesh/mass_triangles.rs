//! Mass-property quads use their shorter spatial diagonal, not display wires.
use super::*;

pub(super) fn split(vertices: &[Point3], [a, b, c, d]: [u32; 4]) -> [[u32; 3]; 2] {
    let (pa, pb, pc, pd) = (
        vertices[a as usize],
        vertices[b as usize],
        vertices[c as usize],
        vertices[d as usize],
    );
    let ac_first = match (pa.distance_to(pc), pb.distance_to(pd)) {
        (Ok(ac), Ok(bd))
            if ac.is_normal()
                && bd.is_normal()
                && (ac - bd).abs() > 16. * Real::EPSILON * ac.max(bd) =>
        {
            ac < bd
        }
        _ => {
            // Distances that overflow or overlap at rounding precision retain
            // an exact, translation-independent comparison, including ties.
            !crate::point::compare_chords([pa, pc], [pb, pd]).is_gt()
        }
    };
    if ac_first {
        [[a, b, c], [a, c, d]]
    } else {
        [[a, b, d], [b, c, d]]
    }
}

impl TriangleMesh {
    pub(crate) fn mass_triangles(&self) -> impl Iterator<Item = [Point3; 3]> + '_ {
        self.mass_triangle_indices()
            .map(|t| t.map(|i| self.vertices[i as usize]))
    }

    pub(crate) fn mass_triangle_indices(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.faces.iter().flat_map(|face| {
            let triangles = match *face {
                MeshFace::Triangle(t) => [Some(t), None],
                MeshFace::Quad(q) => split(&self.vertices, q).map(Some),
            };
            triangles.into_iter().flatten()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_diagonals_keep_ac_and_overflow_does_not_hide_the_shorter_chord() {
        let make = |p: [Real; 3]| Point3::try_from(p).unwrap();
        let tie = [[-2., 0., 0.], [0., -1., 0.], [3., 0., 0.], [0., 2., 4.]].map(make);
        assert_eq!(split(&tie, [0, 1, 2, 3]), [[0, 1, 2], [0, 2, 3]]);
        assert_eq!(split(&tie, [1, 2, 3, 0]), [[1, 2, 3], [1, 3, 0]]);
        let huge = [
            [-1e308, 0., 0.],
            [0., -8e307, 0.],
            [1e308, 0., 0.],
            [0., 8e307, 0.],
        ]
        .map(make);
        assert_eq!(split(&huge, [0, 1, 2, 3]), [[0, 1, 3], [1, 2, 3]]);
        // Hardware-rounded hypotenuses coincide; the exact squared lengths do not.
        let tiny = [[0., 0., 0.], [0., 2., 0.], [1., 1e-10, 0.], [1., 2., 0.]].map(make);
        assert_eq!(split(&tiny, [0, 1, 2, 3]), [[0, 1, 3], [1, 2, 3]]);
        // Equal exact lengths sqrt(29)*quantum, but chained subnormal hypots
        // round to different lengths and the relative error bound underflows.
        let quantum = Real::from_bits(1);
        let subnormal = [[0., 0., 0.], [0., 0., 1.], [2., 3., 4.], [4., 3., 3.]]
            .map(|p| make(p.map(|x| x * quantum)));
        assert_eq!(split(&subnormal, [0, 1, 2, 3]), [[0, 1, 2], [0, 2, 3]]);
    }
}
