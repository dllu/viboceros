#![cfg(feature = "native")]
use viboceros_geometry::*;
use viboceros_smlib::offset_surface;

fn source() -> NurbsSurface {
    let weights = [1., 1.4, 0.8, 1.2, 1.1, 0.7, 1.6, 1., 1.3, 1., 0.9, 1.5];
    let mut controls = Vec::new();
    for (j, y) in [0., 1., 3.].into_iter().enumerate() {
        for (i, x) in [0., 0.7, 2.8, 4.].into_iter().enumerate() {
            controls.push(
                WeightedPoint3::try_new(
                    Point3::try_new(x, y, (i as f64 - j as f64) * 0.17 + (i * j) as f64 * 0.12)
                        .unwrap(),
                    weights[j * 4 + i],
                )
                .unwrap(),
            );
        }
    }
    NurbsSurface::try_new_rational(
        2,
        2,
        4,
        3,
        controls,
        vec![2., 2., 2., 3., 7., 7., 7.],
        vec![-3., -3., -3., 5., 5., 5.],
    )
    .unwrap()
}

#[test]
fn smooth_rational_patches_retain_nonunit_charts_and_signed_normal_images() {
    let source = source();
    let before = source.clone();
    for interval in [2. ..=3., 3. ..=7.] {
        let patch = source.try_trimmed(interval, -3. ..=5.).unwrap();
        let original = patch.clone();
        for distance in [-0.3, 0.5] {
            let result = offset_surface(&patch, distance, 1e-8).unwrap();
            assert_eq!(result.domain_u(), patch.domain_u());
            assert_eq!(result.domain_v(), patch.domain_v());
            for i in 0..=16 {
                for j in 0..=16 {
                    let u = patch.parameter_at_u(i as f64 / 16.).unwrap();
                    let v = patch.parameter_at_v(j as f64 / 16.).unwrap();
                    let ideal = patch
                        .evaluate(u, v)
                        .unwrap()
                        .translated(
                            patch
                                .normal_at(u, v)
                                .unwrap()
                                .as_vector()
                                .scaled(distance)
                                .unwrap(),
                        )
                        .unwrap();
                    let gap = ideal.distance_to(result.evaluate(u, v).unwrap()).unwrap();
                    assert!(gap <= 1e-8, "distance {distance}, uv {u},{v}, gap {gap}");
                }
            }
            assert_eq!(patch, original);
        }
    }
    assert_eq!(source, before);
}

#[test]
fn invalid_fits_leave_the_source_usable() {
    let source = source().try_trimmed(2. ..=3., -3. ..=5.).unwrap();
    let before = source.clone();
    for distance in [0., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(offset_surface(&source, distance, 1e-8).is_err());
    }
    for approximation in [0., -1., f64::NAN, f64::INFINITY, 1e-300] {
        assert!(offset_surface(&source, 0.5, approximation).is_err());
    }
    assert_eq!(source, before);
    let result = offset_surface(&source, 0.5, 1e-8).unwrap();
    assert_eq!(result.domain_u(), source.domain_u());
}
