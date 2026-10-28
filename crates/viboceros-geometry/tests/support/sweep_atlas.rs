//! Preserve every V patch, including both limits of each captured join.
use viboceros_geometry::NurbsSurface;

pub fn join_v(mut pieces: Vec<NurbsSurface>) -> NurbsSurface {
    pieces.sort_by(|a, b| a.domain_v().start().total_cmp(b.domain_v().start()));
    let first = &pieces[0];
    let (du, dv, nu) = (
        first.degree_u(),
        first.degree_v(),
        first.control_point_count_u(),
    );
    let mut controls = Vec::new();
    let mut knots = Vec::new();
    let mut end = None;
    let mut nv = 0;
    for s in &pieces {
        assert_eq!(s.degree_u(), du);
        assert_eq!(s.degree_v(), dv);
        assert_eq!(s.knots_u(), first.knots_u());
        assert_eq!(s.control_point_count_u(), nu);
        assert!(s.knots_v()[..=dv].iter().all(|k| k == s.domain_v().start()));
        assert!(
            s.knots_v()[s.control_point_count_v()..]
                .iter()
                .all(|k| k == s.domain_v().end())
        );
        if let Some(end) = end {
            assert_eq!(*s.domain_v().start(), end);
            knots.extend_from_slice(&s.knots_v()[dv + 1..]);
        } else {
            knots.extend_from_slice(s.knots_v());
        }
        end = Some(*s.domain_v().end());
        controls.extend_from_slice(s.control_points());
        nv += s.control_point_count_v();
    }
    NurbsSurface::try_new_rational(du, dv, nu, nv, controls, first.knots_u().to_vec(), knots)
        .unwrap()
}
