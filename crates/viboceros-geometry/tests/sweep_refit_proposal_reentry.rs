//! Independently authored public-frame candidates, not native intermediates.
#[path = "support/section_refit.rs"]
mod section_refit;

#[test]
fn public_frame_candidate_reentry_matches_complete_native_images() {
    section_refit::require_complete_family("sweep1_refit_proposal_reentry", 42);
}

#[test]
fn no_fit_opposed_preparation_matches_complete_native_images() {
    section_refit::require_complete_family("sweep1_refit_opposed_preparation", 14);
}

#[test]
fn comparison_accepts_equivalent_negative_gauges_and_proves_distinct_images() {
    use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
    let curve = |gauge, offset| {
        NurbsCurve::try_new_rational(
            3,
            [[0., 0., 0.], [1., 2., 0.], [3., -1., 0.], [4., 0., 0.]]
                .map(|[x, y, z]| {
                    WeightedPoint3::try_new(Point3::try_new(x, y + offset, z).unwrap(), gauge)
                        .unwrap()
                })
                .to_vec(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap()
    };
    let source = curve(-7., 0.);
    let same = section_refit::compare(&source, &curve(2., 0.));
    assert!(same["complete_image_bound"].as_f64().unwrap() <= 1e-7);
    let distinct = section_refit::compare(&source, &curve(-2., 1.));
    assert!(distinct["complete_image_bound"].is_null());
    assert!(
        distinct["complete_image_separation_lower_bound"]
            .as_f64()
            .unwrap()
            > 1e-7
    );
}
