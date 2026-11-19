use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn circle(tolerance: Tolerance) -> NurbsCurve {
    Circle3::try_new(
        p(0., 0., 0.),
        1.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

fn cutter(width: f64, depth: f64, tilted: bool, tolerance: Tolerance) -> Brep {
    // The rectangle's natural chart faces in -Z.
    let surface = NurbsSurface::try_bilinear([
        p(-width, depth, 1.),
        p(width, depth, 1.),
        p(width, -depth, if tilted { 1.05 } else { 1. }),
        p(-width, -depth, if tilted { 1.05 } else { 1. }),
    ])
    .unwrap();
    Brep::try_surface_face(surface, tolerance).unwrap()
}

#[test]
fn tapered_parallel_cut_keeps_the_oppositely_oriented_cutter_chart() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let curve = circle(tolerance);
    let profile = KernelCurve::from_nurbs_for_modeling(&curve).unwrap();
    // The rectangle encloses the section at z=1, within the footprint at z=4.
    let target = cutter(1.2, 1.2, false, tolerance);
    let displacement = Vector3::try_new(0., 0., 4.).unwrap();
    let result = produce(
        &curve,
        &profile,
        &target,
        displacement,
        -8.,
        TaperedExtrusionOptions {
            solid: true,
            ..Default::default()
        },
        tolerance,
    )
    .unwrap()
    .expect("covered convex section");
    assert!(result.is_solid());
    assert_eq!((result.faces().len(), result.edges().len()), (3, 3));
    let cap = &target.faces()[0];
    assert!(
        result
            .faces()
            .iter()
            .any(|face| face.surface() == cap.surface() && face.is_reversed())
    );
    let radius = 1. + 8_f64.to_radians().tan();
    let volume = std::f64::consts::PI * (1. + radius + radius * radius) / 3.;
    assert!((result.signed_volume(tolerance).unwrap().abs() - volume).abs() < 1e-5);
}

#[test]
fn tapered_parallel_cut_defers_partial_coverage_and_oblique_cutters() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let curve = circle(tolerance);
    let profile = KernelCurve::from_nurbs_for_modeling(&curve).unwrap();
    for target in [
        cutter(1.2, 1.1, false, tolerance),
        cutter(3., 3., true, tolerance),
    ] {
        assert!(
            produce(
                &curve,
                &profile,
                &target,
                Vector3::try_new(0., 0., 4.).unwrap(),
                -8.,
                TaperedExtrusionOptions::default(),
                tolerance,
            )
            .unwrap()
            .is_none()
        );
    }
}
