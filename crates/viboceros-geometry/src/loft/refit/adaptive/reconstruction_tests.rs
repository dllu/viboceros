use super::*;
use crate::curve_fit::g1::reconstruction_deviation_bound;
use serde_json::Value;

fn source() -> NurbsCurve {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let input: Value = serde_json::from_slice(
        &std::fs::read(root.join("capture_sources/sweep1_refit_c1_profiles.json")).unwrap(),
    )
    .unwrap();
    let d = &input["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "nurbs-circle-1e-06")
        .unwrap()["curve"];
    NurbsCurve::try_new_rational(
        3,
        d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                crate::WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[Real; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(d["knots"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn a_reconstructed_c1_profile_has_a_complete_bound_across_independent_clocks() {
    let source = source();
    let before = source.clone();
    let proposal = certified_joined_refit_with_scope(&source, 1e-6, 0., FitPolicy::sweep(2))
        .unwrap()
        .expect("a valid reconstructed profile needs target-knot and tangent proposals");
    assert!(proposal.maximum_deviation_bound <= 1e-6);
    for (source_domain, target_domain) in [(0. ..=1., -3. ..=17.), (-7. ..=13., 100. ..=107.)] {
        let a = source.try_reparameterized(source_domain).unwrap();
        let b = proposal.curve.try_reparameterized(target_domain).unwrap();
        let unchanged = (a.clone(), b.clone());
        let bound = reconstruction_deviation_bound(&a, &b, 1e-6)
            .unwrap()
            .expect("source and target native parameter clocks must be normalized independently");
        assert!(bound <= 1e-6);
        assert_eq!((a, b), unchanged);
    }
    let displaced = proposal
        .curve
        .transformed(crate::AffineTransform3::from_translation(
            crate::Vector3::try_new(0., 0., 4e-6).unwrap(),
        ))
        .unwrap();
    assert!(
        reconstruction_deviation_bound(&source, &displaced, 1e-6)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn reconstructed_correspondences_cannot_erase_a_retraced_source_visit() {
    let source = NurbsCurve::try_new(
        5,
        [0., 1., 1., 0., 0., 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    let candidate = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        reconstruction_deviation_bound(&source, &candidate, 1e-6)
            .unwrap()
            .is_none()
    );
}
