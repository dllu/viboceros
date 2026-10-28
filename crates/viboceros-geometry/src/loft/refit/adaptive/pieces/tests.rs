use super::*;

mod loops;

fn source() -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [1.5, 5e-6, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap()
}

#[test]
fn an_unsampled_corner_cannot_spend_the_c1_reconstruction_reserve() {
    for degree in [1, 3, 5] {
        for shifted in [false, true] {
            let mut source = source().try_change_degree(degree, false).unwrap();
            if shifted {
                source = source.try_reparameterized(-3. ..=17.).unwrap();
            }
            let breaks = source
                .parametric_discontinuity_parameters(crate::CurveParametricContinuity::C1)
                .unwrap();
            assert_eq!(breaks.len(), 1);
            let before = source.clone();
            let tolerance = 2.5e-7;
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT)
                    .unwrap();
            let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
            let lookup = sampler
                .parameters_at_incremental_normalized_lengths(
                    &fractions,
                    crate::curve_fit::g1::FITTING_BATCH_TOLERANCE,
                )
                .unwrap();
            let parameters = fractions.map(|f| lookup_parameter(&lookup, 0.625 + f * 0.0625));
            let candidate = crate::curve_fit::g1::initial_cubic_from_frames(
                source.evaluate(parameters[0]).unwrap(),
                source.evaluate(parameters[8]).unwrap(),
                source
                    .tangent_at_on_side(parameters[0], ParameterSide::Right)
                    .unwrap(),
                source
                    .tangent_at_on_side(parameters[8], ParameterSide::Right)
                    .unwrap(),
            )
            .unwrap();
            assert!(sampled_fit(&source, &candidate, &parameters, tolerance * 0.5).unwrap());
            let corner = source.evaluate(breaks[0]).unwrap();
            let closest = candidate
                .prepare_closest_parameter_search()
                .unwrap()
                .closest_parameter(corner, Tolerance::DEFAULT)
                .unwrap();
            let distance = corner
                .distance_to(candidate.evaluate(closest).unwrap())
                .unwrap();
            assert!(distance > tolerance * 0.5 && distance < tolerance);
            let proposal = construct(
                &source,
                tolerance,
                1_f64.to_radians(),
                SplitClock::SourceLookup,
            )
            .unwrap()
            .unwrap();
            assert!(proposal.breaks.contains(&0.65625));
            assert_eq!(source, before);
        }
    }
}

#[test]
fn an_exact_ending_break_keeps_its_existing_full_interval_bound() {
    let source = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [2., 1e-5, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let tolerance = (4. / 27.) * 1e-5 / 0.505;
    let candidate = crate::curve_fit::g1::initial_cubic_from_frames(
        source.evaluate(0.).unwrap(),
        source.evaluate(1.).unwrap(),
        source.tangent_at_on_side(0., ParameterSide::Right).unwrap(),
        source.tangent_at_on_side(1., ParameterSide::Right).unwrap(),
    )
    .unwrap();
    let parameters = (0..=8).map(|i| i as Real / 8.).collect::<Vec<_>>();
    assert!(sampled_fit(&source, &candidate, &parameters, tolerance * 0.5).unwrap());
    let original = source.try_trimmed(0. ..=1.).unwrap();
    assert!(
        !crate::curve_fit::g1::within_deviation(&original, &candidate, tolerance * 0.5).unwrap()
    );
    assert!(crate::curve_fit::g1::within_deviation(&original, &candidate, tolerance).unwrap());
    let proposal = construct(
        &source,
        tolerance,
        1_f64.to_radians(),
        SplitClock::SourceLookup,
    )
    .unwrap()
    .unwrap();
    assert_eq!(proposal.breaks, [0., 0.5, 1.]);
}

#[test]
fn each_original_source_piece_keeps_a_complete_half_tolerance_reserve() {
    use serde_json::Value;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/loft_refit_unequal_sampling.json")).unwrap(),
    )
    .unwrap();
    let mut pieces = 0;
    let operations = query["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 24);
    for operation in operations {
        let definition = &operation["curves"][0];
        let source = NurbsCurve::try_new_rational(
            1,
            definition["control_points"]
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
            serde_json::from_value(definition["knots"].clone()).unwrap(),
        )
        .unwrap();
        let tolerance = operation["refit_tolerance"].as_f64().unwrap();
        let before = source.clone();
        let proposal = construct(
            &source,
            tolerance,
            1_f64.to_radians(),
            SplitClock::SourceLookup,
        )
        .unwrap()
        .unwrap();
        for (parameters, fractions) in proposal
            .source_parameters
            .windows(2)
            .zip(proposal.breaks.windows(2))
        {
            let original = source.try_trimmed(parameters[0]..=parameters[1]).unwrap();
            let candidate = proposal
                .curve
                .try_trimmed(fractions[0]..=fractions[1])
                .unwrap();
            assert!(
                crate::curve_fit::g1::within_deviation(&original, &candidate, tolerance * 0.5)
                    .unwrap(),
                "{}",
                operation["id"]
            );
            pieces += 1;
        }
        assert_eq!(source, before);
    }
    println!("HALF_RESERVE sources=24 certified_pieces={pieces}");
}
