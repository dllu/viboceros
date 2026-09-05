//! Serialization must preserve Euclidean controls, not merely finite payloads.

use crate::*;
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, PolyCurve3, Tolerance, WeightedPoint3};

mod brep;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn model(geometry: ThreeDmGeometry) -> ThreeDmModel {
    ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Rational".into(),
            color: [1, 2, 3],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(geometry, 0)],
    )
}

fn round_trip(geometry: ThreeDmGeometry) -> ThreeDmGeometry {
    let model = model(geometry);
    let original = model.clone();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rational.3dm");
    write_3dm_file(&path, &model).unwrap();
    assert_eq!(model, original);
    let mut decoded = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.unsupported_object_count(), 0);
    assert_eq!(decoded.objects.len(), 1);
    decoded.objects.pop().unwrap().geometry
}

fn curve(scale: f64, weight: f64) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (p(scale, scale, 0.0), weight),
            (p(2.0 * scale, 3.0 * scale, scale), weight / 2.0),
            (p(3.0 * scale, scale, 0.0), weight),
        ]
        .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap()
}

fn near(actual: f64, expected: f64) {
    if expected == 0.0 {
        assert_eq!(actual, 0.0);
    } else {
        assert!(
            ((actual - expected) / expected).abs() < 4e-14,
            "{actual:e} != {expected:e}"
        );
    }
}

fn controls_near(actual: &[WeightedPoint3], expected: &[WeightedPoint3]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
            near(a, b);
        }
        near(
            a.weight() / actual[0].weight(),
            b.weight() / expected[0].weight(),
        );
    }
}

fn curves_near(actual: &NurbsCurve, expected: &NurbsCurve) {
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(actual.knots(), expected.knots());
    controls_near(actual.control_points(), expected.control_points());
    for i in 0..=64 {
        let t = expected.parameter_at(i as f64 / 64.0).unwrap();
        for (a, b) in actual
            .evaluate(t)
            .unwrap()
            .to_array()
            .into_iter()
            .zip(expected.evaluate(t).unwrap().to_array())
        {
            near(a, b);
        }
    }
}

#[test]
fn tiny_products_must_not_silently_erase_euclidean_curve_coordinates() {
    let source = curve(1e-200, 1e-200);
    let ThreeDmGeometry::NurbsCurve(decoded) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    curves_near(&decoded, &source);
}

#[test]
fn overflowing_products_use_a_common_scale_instead_of_rejecting_valid_geometry() {
    let source = curve(1e200, 1e200);
    let ThreeDmGeometry::NurbsCurve(decoded) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    curves_near(&decoded, &source);
}

#[test]
fn subnormal_weights_must_not_overflow_opennurbs_reciprocal_dehomogenization() {
    let source = curve(1.0, 1e-320);
    let ThreeDmGeometry::NurbsCurve(decoded) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    curves_near(&decoded, &source);
}

#[test]
fn polycurve_codec_uses_the_same_safe_rational_conversion() {
    let source = curve(1e-200, -1e-200);
    let polycurve = PolyCurve3::try_new(vec![source.clone()]).unwrap();
    let ThreeDmGeometry::PolyCurve(decoded) = round_trip(ThreeDmGeometry::PolyCurve(polycurve))
    else {
        panic!("expected polycurve")
    };
    let viboceros_geometry::CurveSegment3::NurbsCurve(decoded) = &decoded.segments()[0] else {
        panic!("expected NURBS leaf")
    };
    curves_near(decoded, &source);
}

#[test]
fn free_surface_conversion_preserves_tiny_euclidean_controls() {
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(1e-200, 1e-200, 0.0),
            p(2e-200, 1e-200, 0.0),
            p(1e-200, 2e-200, 0.0),
            p(2e-200, 2e-200, 1e-200),
        ]
        .map(|point| WeightedPoint3::try_new(point, 1e-200).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let ThreeDmGeometry::NurbsSurface(decoded) =
        round_trip(ThreeDmGeometry::NurbsSurface(source.clone()))
    else {
        panic!("expected NURBS surface")
    };
    controls_near(decoded.control_points(), source.control_points());
}

#[test]
fn importer_recovers_subnormal_homogeneous_controls_without_forming_a_reciprocal() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pre_scale_subnormal_weights.3dm");
    let decoded = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.unsupported_object_count(), 0);
    assert_eq!(decoded.objects.len(), 4);
    let expected = curve(1.0, 1e-320);
    for object in decoded.objects {
        let ThreeDmGeometry::NurbsCurve(actual) = object.geometry else {
            panic!("expected NURBS")
        };
        assert_eq!(actual.control_points()[0].weight(), 1e-320);
        curves_near(&actual, &expected);
        let ThreeDmGeometry::NurbsCurve(rewritten) =
            round_trip(ThreeDmGeometry::NurbsCurve(actual))
        else {
            panic!("expected NURBS")
        };
        curves_near(&rewritten, &expected);
    }
}

#[test]
fn irreconcilable_products_fail_without_replacing_existing_data() {
    // One span requires opposite global rescalings at its two controls.
    let curve = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(1e200, 0.0, 0.0), 1e200).unwrap(),
            WeightedPoint3::try_new(p(1e-200, 0.0, 0.0), 1e-200).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let model = model(ThreeDmGeometry::NurbsCurve(curve));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.3dm");
    std::fs::write(&path, b"existing file").unwrap();
    assert!(
        matches!(write_3dm_file(&path, &model), Err(ThreeDmError::Native(message)) if message.contains("common binary weight scale"))
    );
    assert_eq!(std::fs::read(path).unwrap(), b"existing file");
}

#[test]
fn binary_scaling_handles_subnormal_endpoints_and_preserves_ordinary_weights() {
    for (coordinate, weight) in [
        (f64::from_bits(1), f64::from_bits(2)),
        (f64::from_bits(1), 1e-200),
        (1.0, -1e308),
        (1.0, 1e-320),
        (1e-300, 1e-300),
        (1e100, -1e250),
        (1.0, 1.0),
        (1.0, 1e-200),
        (1.0, 1e200),
    ] {
        let source = curve(coordinate, weight);
        let ThreeDmGeometry::NurbsCurve(decoded) =
            round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
        else {
            panic!("expected NURBS")
        };
        curves_near(&decoded, &source);
        if coordinate == 1.0 && [1.0, 1e-200, 1e200].contains(&weight) {
            assert_eq!(decoded.control_points(), source.control_points());
        }
    }
}

#[test]
fn exact_subnormal_products_are_allowed_when_no_all_normal_encoding_exists() {
    let source = NurbsCurve::try_new_rational(
        1,
        [p(f64::from_bits(1), 0.0, 0.0), p(1e300, 0.0, 0.0)]
            .map(|point| WeightedPoint3::try_new(point, 1e100).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let ThreeDmGeometry::NurbsCurve(actual) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    assert_eq!(
        actual.control_points()[0].point(),
        source.control_points()[0].point()
    );
    curves_near(&actual, &source);
}

#[test]
fn full_order_decomposition_and_binary_scaling_preserve_independent_pieces() {
    let source = NurbsCurve::try_new_rational(
        1,
        [
            (1e200, 1e200),
            (2e200, 1e200),
            (2e200, 1e-200),
            (3e200, 1e-200),
        ]
        .map(|(x, weight)| WeightedPoint3::try_new(p(x, 0.0, 0.0), weight).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let ThreeDmGeometry::PolyCurve(actual) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected polycurve")
    };
    assert_eq!(actual.parameters(), &[0.0, 1.0, 2.0]);
    for (a, e) in actual
        .segments()
        .iter()
        .zip(source.try_split_at_full_order_knots().unwrap())
    {
        let viboceros_geometry::CurveSegment3::NurbsCurve(a) = a else {
            panic!("expected NURBS leaf")
        };
        curves_near(a, &e);
    }
}

#[test]
fn finite_reciprocals_in_the_lowest_usable_weight_bin_are_not_rejected() {
    let source = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(1.0, 0.0, 0.0), f64::MIN_POSITIVE * 0.75).unwrap(),
            WeightedPoint3::try_new(p(2.0, 0.0, 0.0), 1e308).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let ThreeDmGeometry::NurbsCurve(actual) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    for (a, e) in actual.control_points().iter().zip(source.control_points()) {
        assert_eq!(a.point(), e.point());
        assert_eq!(a.weight(), e.weight() * 0.5);
    }
}

#[test]
fn rounding_to_the_smallest_subnormal_is_allowed_only_when_coordinates_recover() {
    let weight = 1.5 * 2.0_f64.powi(100);
    let source = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(f64::from_bits(1), 0.0, 0.0), weight).unwrap(),
            WeightedPoint3::try_new(p(6e307, 0.0, 0.0), 2.0 * weight).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let ThreeDmGeometry::NurbsCurve(actual) =
        round_trip(ThreeDmGeometry::NurbsCurve(source.clone()))
    else {
        panic!("expected NURBS")
    };
    curves_near(&actual, &source);
    assert_eq!(actual.control_points()[0].weight(), 0.75);
    assert_eq!(actual.control_points()[0].point().x(), f64::from_bits(1));
}
