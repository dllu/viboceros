//! Full-image bound for positive rational tensors with identical normalized bases.
use viboceros_geometry::NurbsSurface;

pub fn complete_bound(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    let a = a.try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
    let b = b.try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
    assert_eq!((a.degree_u(), a.degree_v()), (b.degree_u(), b.degree_v()));
    assert_eq!(a.knots_u(), b.knots_u());
    assert_eq!(a.knots_v(), b.knots_v());
    assert_eq!(a.control_points().len(), b.control_points().len());
    let anchor = b.control_points()[0].point();
    let (mut delta, mut weight_delta, mut radius, mut minimum) =
        (0_f64, 0_f64, 0_f64, f64::INFINITY);
    for (ca, cb) in a.control_points().iter().zip(b.control_points()) {
        assert!(ca.weight() > 0. && cb.weight() > 0.);
        delta = delta.max(ca.point().distance_to(cb.point()).unwrap());
        weight_delta = weight_delta.max((ca.weight() - cb.weight()).abs());
        radius = radius.max(anchor.distance_to(cb.point()).unwrap());
        minimum = minimum.min(ca.weight());
    }
    // Shared nonnegative tensor basis, partition of unity: the point
    // contribution is <= delta; normalized weight changes contribute
    // <= 2 * max_weight_delta * hull_radius / min_weight everywhere.
    delta + 2. * weight_delta * radius / minimum + 128. * f64::EPSILON * radius.max(1.)
}
