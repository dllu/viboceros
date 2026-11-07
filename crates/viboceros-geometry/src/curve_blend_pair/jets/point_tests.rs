//! A single regular endpoint uses a degree-independent chord-speed default.
use super::*;
#[test]
fn point_endpoints_use_chord_speed_bulges_and_explicit_handles_at_every_order() {
    let positions = [[0., 0., 0.], [8., 4., 2.]].map(|p| Point3::try_from(p).unwrap());
    let zero = Vector3::try_new(0., 0., 0.).unwrap();
    let tangent = Vector3::try_new(0., 1., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let chord = positions[0].distance_to(positions[1]).unwrap();
    for source in 0..2 {
        let jets = std::array::from_fn(|i| {
            CurveBlendJet::new(positions[i], (i == source).then_some(tangent), zero)
                .with_higher_derivatives(zero, zero)
        });
        for order in 1..=4 {
            let mut orders = [0; 2];
            orders[source] = order;
            for (handle, bulge) in [(None, 1.), (None, 1.6), (Some(2.2), 9.)] {
                let mut handles = [None; 2];
                handles[source] = handle;
                let mut bulges = [1.; 2];
                bulges[source] = bulge;
                let curve = try_blend_curve_from_endpoint_jets(
                    jets,
                    CurveBlendJetOptions {
                        orders,
                        handles,
                        bulges,
                        endpoint_specific: true,
                    },
                    Tolerance::DEFAULT,
                )
                .unwrap();
                let (_, jet) = curve
                    .evaluate_with_fourth_derivatives_on_side(source as Real, ParameterSide::Right)
                    .unwrap();
                let speed = handle.map_or(chord * bulge, |v| v * Real::from(order + 1));
                assert_eq!(curve.degree(), usize::from(order + 1));
                for (actual, expected) in jet[0].to_array().into_iter().zip([0., speed, 0.]) {
                    assert!((actual - expected).abs() < 1e-11);
                }
                for derivative in jet.iter().take(usize::from(order)).skip(1) {
                    assert!(derivative.length().unwrap() < 1e-9);
                }
            }
        }
    }
}
