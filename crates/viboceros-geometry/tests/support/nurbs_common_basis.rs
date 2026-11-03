//! Exact refinement into a common normalized tensor basis.
use viboceros_geometry::NurbsSurface;

pub fn common_basis(a: &NurbsSurface, b: &NurbsSurface) -> (NurbsSurface, NurbsSurface) {
    let mut a = a.try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
    let mut b = b.try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
    let degrees = (
        a.degree_u().max(b.degree_u()),
        a.degree_v().max(b.degree_v()),
    );
    if (a.degree_u(), a.degree_v()) != degrees {
        a = a.try_change_degree(degrees.0, degrees.1, false).unwrap();
    }
    if (b.degree_u(), b.degree_v()) != degrees {
        b = b.try_change_degree(degrees.0, degrees.1, false).unwrap();
    }
    for axis in 0..2 {
        let knots = |s: &NurbsSurface| {
            if axis == 0 {
                s.knots_u().to_vec()
            } else {
                s.knots_v().to_vec()
            }
        };
        let mut groups = Vec::new();
        for s in [&a, &b] {
            for g in knots(s).chunk_by(|a, b| a == b) {
                if g[0] > 0. && g[0] < 1. {
                    groups.push((g[0], g.len()));
                }
            }
        }
        groups.sort_by(|a, b| a.0.total_cmp(&b.0));
        for group in groups.chunk_by(|a, b| a.0 == b.0) {
            let t = group[0].0;
            let multiplicity = group.iter().map(|g| g.1).max().unwrap();
            for s in [&mut a, &mut b] {
                *s = if axis == 0 {
                    s.try_insert_knot_u(t, multiplicity)
                } else {
                    s.try_insert_knot_v(t, multiplicity)
                }
                .unwrap();
            }
        }
    }
    (a, b)
}
