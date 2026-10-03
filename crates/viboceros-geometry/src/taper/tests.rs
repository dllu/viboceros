use super::*;
use crate::{LineSegment, Vector3, WeightedPoint3};
use serde_json::Value;

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn morph(op: &Value) -> Result<TaperPointMorph, GeometryError> {
    TaperPointMorph::try_new(
        p(&op["start"]),
        p(&op["end"]),
        op["start_radius"].as_f64().unwrap(),
        op["end_radius"].as_f64().unwrap(),
        op["flat"].as_bool().unwrap(),
        op["infinite"].as_bool().unwrap(),
    )
}
fn near(actual: Point3, expected: Point3, absolute: Real, relative: Real, label: &str) {
    for (a, b) in actual.to_array().into_iter().zip(expected.to_array()) {
        assert!(
            (a - b).abs() <= absolute + relative * b.abs(),
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
fn taper_maps_match_fifty_one_native_sdk_cases_and_validity_boundaries() {
    let mut count = 0;
    for (input, output) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_boundary_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_boundary_points.json"),
        ),
    ] {
        let input: Value = serde_json::from_str(input).unwrap();
        let output: Value = serde_json::from_str(output).unwrap();
        assert_eq!(
            input["operations"].as_array().unwrap().len(),
            output["results"].as_array().unwrap().len()
        );
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(output["results"].as_array().unwrap())
        {
            count += 1;
            let label = op["id"].as_str().unwrap();
            assert_eq!(op["id"], row["id"]);
            let morph = morph(op);
            assert_eq!(
                morph.is_ok(),
                row["value"]["valid"].as_bool().unwrap(),
                "{label}"
            );
            let native = row["value"]["points"].as_array().unwrap();
            assert_eq!(op["points"].as_array().unwrap().len(), native.len());
            for (source, expected) in op["points"].as_array().unwrap().iter().zip(native) {
                let source = p(source);
                let actual = morph
                    .as_ref()
                    .map_or(source, |m| m.morph_point(source).unwrap());
                near(actual, p(expected), 1e-11, 1e-12, label);
            }
        }
    }
    assert_eq!(count, 51);
}

#[test]
fn taper_point_maps_and_grouped_rigid_pose_match_six_actual_commands() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/taper_command_points.json"
    ))
    .unwrap();
    let output: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/taper_command_points.json"
    ))
    .unwrap();
    assert_eq!(output["results"].as_array().unwrap().len(), 6);
    for (op, row) in input["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(output["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        assert_eq!(row["value"]["success"], true, "{label}");
        let morph = morph(op).unwrap();
        let points = op["points"].as_array().unwrap();
        let native = row["value"]["after"].as_array().unwrap();
        let pose = (op["rigid"] == true).then(|| morph.rigid_transform(point(2., 1., 5.)).unwrap());
        for (source, expected) in points.iter().zip(&native[native.len() - points.len()..]) {
            let actual = pose.map_or_else(
                || morph.morph_point(p(source)).unwrap(),
                |pose| pose.transform_point(p(source)).unwrap(),
            );
            near(
                actual,
                p(&expected["point"]),
                if pose.is_some() { 1e-7 } else { 1e-11 },
                0.,
                label,
            );
        }
    }
}

#[test]
fn taper_extreme_ranges_keep_representable_images_and_reject_overflow() {
    let frame = Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let huge = TaperPointMorph::try_from_frame(frame, 1., 1e-308, 1e308, false, true).unwrap();
    near(
        huge.morph_point(point(1e-308, 0., 1.)).unwrap(),
        point(1e308, 0., 1.),
        0.,
        1e-14,
        "overflowing ratio",
    );
    assert!(huge.morph_point(point(1., 0., 1.)).is_err());
    assert_eq!(
        huge.morph_point(point(0., 0., 1.)).unwrap(),
        point(0., 0., 1.)
    );
    let tiny = TaperPointMorph::try_from_frame(frame, 1., 1e308, 1e-308, false, true).unwrap();
    near(
        tiny.morph_point(point(1e308, 0., 1.)).unwrap(),
        point(1e-308, 0., 1.),
        f64::from_bits(1) * 2.,
        1e-14,
        "contracting ratio",
    );
    let offset = frame.with_origin(point(-Real::MAX, 0., 0.));
    let contraction = TaperPointMorph::try_from_frame(offset, 1., 1., 0.5, false, true).unwrap();
    assert_eq!(
        contraction.morph_point(point(Real::MAX, 1., 1.)).unwrap(),
        point(0., 0.5, 1.)
    );
    let short = TaperPointMorph::try_from_frame(frame, 1e-308, 2., 1., false, true).unwrap();
    assert_eq!(
        short.morph_point(point(0., 0., 1e308)).unwrap(),
        point(0., 0., 1e308)
    );
    let long = TaperPointMorph::try_from_frame(frame, 1e308, 1e-308, 1e308, false, true).unwrap();
    near(
        long.morph_point(point(2., 0., 1e-308)).unwrap(),
        point(4., 0., 1e-308),
        1e-14,
        0.,
        "underflowing axial fraction",
    );
    let eased = TaperPointMorph::try_from_frame(frame, 1., 1e-308, 1e308, false, false).unwrap();
    near(
        eased.morph_point(point(1., 0., 1e-308)).unwrap(),
        point(4., 0., 1e-308),
        1e-14,
        0.,
        "underflowing cubic blend",
    );
    let identity = TaperPointMorph::try_from_frame(offset, 1., 1., 1., false, true).unwrap();
    assert_eq!(
        identity.morph_point(point(Real::MAX, 1., 1.)).unwrap(),
        point(Real::MAX, 1., 1.)
    );
    for bad in [Real::NAN, Real::INFINITY, 0., -1.] {
        assert!(TaperPointMorph::try_from_frame(frame, 1., bad, 1., false, false).is_err());
        assert!(TaperPointMorph::try_from_frame(frame, bad, 1., 2., false, false).is_err());
    }
}

#[test]
fn taper_flat_direction_can_be_supplied_independently_of_sdk_basis() {
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Vector3::try_new(-1., 0., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let flat = TaperPointMorph::try_from_frame(frame, 10., 2., 1., true, false).unwrap();
    assert_eq!(
        flat.morph_point(point(2., 3., 10.)).unwrap(),
        point(2., 1.5, 10.)
    );
    assert_eq!(
        flat.morph_point(point(2., 3., -10.)).unwrap(),
        point(2., 3., -10.)
    );
}

#[test]
fn taper_preserved_controls_and_fitted_line_follow_separate_contracts() {
    let morph =
        TaperPointMorph::try_new(point(0., 0., 0.), point(0., 0., 10.), 2., 1., false, false)
            .unwrap();
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(2., 1., 1.), 1.).unwrap(),
            WeightedPoint3::try_new(point(3., 2., 5.), 0.75).unwrap(),
            WeightedPoint3::try_new(point(2., 1., 9.), 1.).unwrap(),
        ],
        vec![3., 3., 3., 7., 7., 7.],
    )
    .unwrap();
    let kept = morph
        .with_preserve_structure(true)
        .morph_nurbs_curve(&curve, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(kept.degree(), curve.degree());
    assert_eq!(kept.knots(), curve.knots());
    for (a, b) in kept.control_points().iter().zip(curve.control_points()) {
        assert_eq!(a.weight(), b.weight());
        assert_eq!(a.point(), morph.morph_point(b.point()).unwrap());
    }
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let line = LineSegment::try_new(point(2., 1., -2.), point(2., 1., 12.), tolerance).unwrap();
    let fitted = morph.morph_line(line, tolerance).unwrap();
    for i in 0..=256 {
        let t = i as Real / 256.;
        near(
            fitted.evaluate(t).unwrap(),
            morph.morph_point(line.evaluate(t).unwrap()).unwrap(),
            1e-6,
            0.,
            "nonlinear line fit",
        );
    }
}
