use viboceros_geometry::{NurbsCurve, Point3, Tolerance};
#[test]
fn pruned_regions_do_not_hide_witness_rounding_error() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(-2., 0.), p(-2., 4.), p(4., 4.)],
        vec![0., 0., 1., 2., 3., 3.],
    )
    .unwrap();
    let point = NurbsCurve::try_new(1, vec![p(1., 1.); 2], vec![0., 0., 1., 1.]).unwrap();
    let tolerance = Tolerance::try_new(f64::from_bits(1), 1e-12, 1e-10).unwrap();
    let result = curve
        .minimum_distance_with_curve_seeded(&point, tolerance, [0., 0.])
        .unwrap();
    println!("precision result {result:?}");
    assert!(
        result.is_none(),
        "sqrt(2) cannot be enclosed by a binary64 upper bound with subnormal value error"
    );
}
