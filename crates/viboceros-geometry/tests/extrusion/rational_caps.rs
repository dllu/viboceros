//! Rational cap boundaries retain their geometry, weights, and topology.
use super::*;

#[test]
fn rational_extrusion_control_net_caps_survive_weight_scaling_shear_and_close_path_ends() {
    use viboceros_geometry::{Circle3, SubD, SubDFromSurfaceMethod, WeightedPoint3};
    let construction_tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-6).unwrap();
    let profile = Circle3::try_new(
        point([0., 0., 0.]),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        construction_tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let shear = AffineTransform3::try_new(
        [[1., 0.3, 0.2], [0., 1., 0.4], [0., 0., 1.]],
        Vector3::try_new(7., -4., 3.).unwrap(),
    )
    .unwrap();
    for (height, transform) in [
        (2., None),
        (-2., None),
        (1e-8, None),
        (2., Some(shear)),
        (-2., Some(shear)),
    ] {
        let mut reference: Option<SubD> = None;
        for scale in [1., 0.3, 2., 7.] {
            let controls = profile
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * scale).unwrap())
                .collect();
            let scaled =
                NurbsCurve::try_new_rational(profile.degree(), controls, profile.knots().to_vec())
                    .unwrap();
            let source = Brep::try_planar_profile_extrusion(
                &scaled,
                frame(),
                height,
                true,
                construction_tolerance,
            )
            .unwrap_or_else(|e| panic!("construct height={height}, scale={scale}: {e}"));
            let source = if let Some(transform) = transform {
                source
                    .transformed(transform, construction_tolerance)
                    .unwrap()
            } else {
                source
            };
            let form = source.extrusion().unwrap().nurbs_form().unwrap();
            let maximum_weight = form
                .surface
                .control_points()
                .iter()
                .map(|p| p.weight())
                .fold(0f64, f64::max);
            assert_eq!(
                maximum_weight, scale,
                "stored profile weights must survive validation"
            );
            let original = source.clone();
            // The thin case has ends closer than this tolerance. Cap
            // membership must still retain the two distinct boundaries.
            let subd = SubD::try_from_extrusion(
                source.extrusion().unwrap(),
                SubDFromSurfaceMethod::ControlPoints,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap_or_else(|e| {
                panic!("height={height}, transform={transform:?}, scale={scale}: {e}")
            });
            assert_eq!(source, original);
            assert_eq!(subd.control_points().len(), 16);
            assert_eq!(subd.edges().len(), 24);
            assert_eq!(subd.faces().len(), 10);
            assert_eq!(subd.faces().iter().filter(|f| f.len() == 8).count(), 2);
            let mut incidence = std::collections::BTreeMap::<[u32; 2], usize>::new();
            for face in subd.faces() {
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    *incidence.entry([a.min(b), a.max(b)]).or_default() += 1;
                }
            }
            assert!(incidence.values().all(|&n| n == 2));
            if let Some(reference) = &reference {
                assert_eq!(subd.faces(), reference.faces());
                for (actual, expected) in
                    subd.control_points().iter().zip(reference.control_points())
                {
                    assert!(actual.distance_to(*expected).unwrap() < 1e-12);
                }
            } else {
                reference = Some(subd);
            }
        }
    }
}

#[test]
fn refined_rational_profiles_keep_caps_and_exact_cylindrical_geometry() {
    use viboceros_geometry::{Circle3, WeightedPoint3};
    let profile = Circle3::try_new(
        point([0., 0., 0.]),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let domain = profile.domain();
    let refined = profile
        .try_insert_knot(*domain.start() + (*domain.end() - *domain.start()) / 8., 1)
        .unwrap();
    for profile in [&profile, &refined] {
        for scale in [0.3, 2., 7., 1e-100, 1e100] {
            let scaled = NurbsCurve::try_new_rational(
                profile.degree(),
                profile
                    .control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * scale).unwrap())
                    .collect(),
                profile.knots().to_vec(),
            )
            .unwrap();
            let original = scaled.clone();
            let result =
                Brep::try_planar_profile_extrusion(&scaled, frame(), 3., true, Tolerance::DEFAULT)
                    .unwrap_or_else(|e| {
                        panic!(
                            "count={}, scale={scale}: {e}",
                            profile.control_points().len()
                        )
                    });
            assert_eq!(scaled, original);
            assert!(result.is_extrusion());
            assert!(result.is_solid());
            assert!(
                (result.signed_volume(Tolerance::DEFAULT).unwrap() - 12. * std::f64::consts::PI)
                    .abs()
                    < 1e-6
            );
            let bounds = result.tight_bounds(Tolerance::DEFAULT).unwrap_or_else(|e| {
                panic!(
                    "bounds count={}, scale={scale}: {e}",
                    profile.control_points().len()
                )
            });
            assert!(bounds.min().distance_to(point([-2., -2., 0.])).unwrap() < 1e-6);
            assert!(bounds.max().distance_to(point([2., 2., 3.])).unwrap() < 1e-6);
        }
    }
}
