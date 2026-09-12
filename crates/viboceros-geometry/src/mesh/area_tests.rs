use super::*;

fn triangle(first: [f64; 3], second: [f64; 3]) -> TriangleMesh {
    TriangleMesh::try_new(
        [[0.; 3], first, second]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![[0, 1, 2]],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
}

#[test]
fn area_can_fit_when_the_cross_product_does_not() {
    let side = 2.0f64.powi(512);
    let mesh = triangle([side, 0., 0.], [0., side, 0.]);
    assert_eq!(mesh.area().unwrap(), 2.0f64.powi(1023));
}

#[test]
fn area_can_fit_when_the_cross_product_length_does_not() {
    let long = 2.0f64.powi(512);
    let short = 1.5 * 2.0f64.powi(511);
    let mesh = triangle([long, 0., 0.], [0., short, short]);
    let expected = (1.5 * 2.0f64.powi(1022)) * 2.0f64.sqrt();
    assert!((mesh.area().unwrap() / expected - 1.0).abs() <= 2.0 * f64::EPSILON);
}

#[test]
fn truly_unrepresentable_area_is_an_error() {
    let side = 2.0f64.powi(513);
    let mesh = triangle([side, 0., 0.], [0., side, 0.]);
    assert!(matches!(mesh.area(), Err(GeometryError::NonFinite { .. })));
}

#[test]
fn small_representable_area_is_not_lost_by_halving_edges() {
    let mesh = triangle([2.0f64.powi(-537), 0., 0.], [0., 2.0f64.powi(-536), 0.]);
    assert_eq!(mesh.area().unwrap(), f64::from_bits(1));
}
