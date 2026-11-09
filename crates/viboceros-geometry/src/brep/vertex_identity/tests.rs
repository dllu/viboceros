use super::*;
fn quad(x: Real, y: Real) -> Brep {
    let p = |dx, dy| Point3::try_new(x + dx, y + dy, 0.).unwrap();
    let a = NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let b = NurbsCurve::try_new(1, vec![p(0., 1.), p(1., 1.)], vec![0., 0., 1., 1.]).unwrap();
    Brep::try_surface_face(
        NurbsSurface::try_ruled_from_curves(&a, &b).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn tip(b: &Brep, p: Point3) -> usize {
    b.vertices().iter().position(|v| v.point() == p).unwrap()
}
#[test]
fn only_declared_tips_share_an_identity_and_geometry_is_retained() {
    let a = quad(0., 0.);
    let b = quad(1., 1.);
    let c = b.clone();
    let p = Point3::try_new(1., 1., 0.).unwrap();
    let pair = (tip(&a, p), a.vertices().len() + tip(&b, p));
    let original = Brep::try_combine(vec![a, b, c], Tolerance::DEFAULT).unwrap();
    let result = original
        .clone()
        .try_identify_vertices(&[pair], Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(result.vertices().len(), original.vertices().len() - 1);
    assert_eq!(
        result.vertices().iter().filter(|v| v.point() == p).count(),
        2
    );
    assert_eq!(result.edge_connected_face_components().len(), 3);
    for (a, b) in original.edges().iter().zip(result.edges()) {
        assert_eq!(a.curve(), b.curve());
    }
    for (a, b) in original.faces().iter().zip(result.faces()) {
        assert_eq!(a.surface(), b.surface());
        for (a, b) in a
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .zip(b.loops().iter().flat_map(|l| l.trims()))
        {
            assert_eq!(a.curve(), b.curve());
        }
    }
}
#[test]
fn model_tolerance_does_not_identify_distinct_nearby_tips() {
    let a = quad(0., 0.);
    let b = quad(1. + 1e-8, 1.);
    let ia = tip(&a, Point3::try_new(1., 1., 0.).unwrap());
    let ib = a.vertices().len() + tip(&b, Point3::try_new(1. + 1e-8, 1., 0.).unwrap());
    let original = Brep::try_combine(vec![a, b], Tolerance::DEFAULT).unwrap();
    assert!(
        original
            .clone()
            .try_identify_vertices(&[(ia, ib)], Tolerance::DEFAULT)
            .is_err()
    );
    assert!(
        original
            .clone()
            .try_identify_vertices(&[(ia, original.vertices().len())], Tolerance::DEFAULT)
            .is_err()
    );
}
#[test]
fn transitive_pairs_cannot_drift_beyond_the_stored_representative() {
    let step = 0.75 * CURVE_COINCIDENCE_ABSOLUTE;
    let parts = (0..3)
        .map(|i| quad(1. + i as Real * step, 1.))
        .collect::<Vec<_>>();
    let mut offset = 0;
    let ids = parts
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let id = offset + tip(b, Point3::try_new(1. + i as Real * step, 1., 0.).unwrap());
            offset += b.vertices().len();
            id
        })
        .collect::<Vec<_>>();
    let b = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
    assert!(
        b.try_identify_vertices(&[(ids[0], ids[1]), (ids[1], ids[2])], Tolerance::DEFAULT)
            .is_err()
    );
}
