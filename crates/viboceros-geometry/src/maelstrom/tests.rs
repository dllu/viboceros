use super::*;
use crate::{LineSegment, WeightedPoint3};
use serde_json::Value;

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn morph(op: &Value) -> Result<MaelstromPointMorph, GeometryError> {
    let normal =
        Vector3::try_from(serde_json::from_value::<[Real; 3]>(op["normal"].clone()).unwrap())?
            .normalized_nonzero()?
            .as_vector();
    let frame = Frame3::try_from_normal(p(&op["origin"]), normal, Tolerance::NUMERICAL_VALIDATION)?;
    MaelstromPointMorph::try_new(
        frame,
        op["radius0"].as_f64().unwrap(),
        op["radius1"].as_f64().unwrap(),
        op["angle_radians"].as_f64().unwrap(),
    )
}
fn near(a: Point3, b: Point3, absolute: Real, relative: Real, label: &str) {
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        assert!(
            (a - b).abs() <= absolute + relative * b.abs(),
            "{label}: {a} != {b}"
        );
    }
}
fn captures() -> Vec<(Value, Value)> {
    [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/maelstrom_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_followup.json"),
            include_str!("../../../../tools/rhino_oracle/observations/maelstrom_followup.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_profile_points.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_profile_points.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_threshold_points.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_threshold_points.json"
            ),
        ),
    ]
    .into_iter()
    .flat_map(|(input, output)| {
        let input: Value = serde_json::from_str(input).unwrap();
        let output: Value = serde_json::from_str(output).unwrap();
        let ops = input["operations"].as_array().unwrap();
        let rows = output["results"].as_array().unwrap();
        assert_eq!(ops.len(), rows.len());
        ops.iter()
            .cloned()
            .zip(rows.iter().cloned())
            .collect::<Vec<_>>()
    })
    .collect()
}

#[test]
fn maelstrom_maps_match_108_sdk_cases_including_relative_thresholds_and_tiny_normals() {
    let mut count = 0;
    for (op, row) in captures() {
        assert_eq!(op["id"], row["id"]);
        if op["op"] != "maelstrom_points" {
            continue;
        }
        count += 1;
        let m = morph(&op);
        let label = op["id"].as_str().unwrap();
        assert_eq!(
            m.is_ok(),
            row["value"]["valid"].as_bool().unwrap(),
            "{label}"
        );
        let sources = op["points"].as_array().unwrap();
        let targets = row["value"]["points"].as_array().unwrap();
        assert_eq!(sources.len(), targets.len());
        for (src, dst) in sources.iter().zip(targets) {
            let src = p(src);
            let actual = m.as_ref().map_or(src, |m| m.morph_point(src).unwrap());
            near(actual, p(dst), 1e-11, 1e-12, label);
        }
    }
    assert_eq!(count, 108);
}

#[test]
fn maelstrom_point_maps_and_grouped_rigid_pose_match_eight_native_commands() {
    let mut count = 0;
    for (op, row) in captures() {
        if op["op"] != "maelstrom_command_points" {
            continue;
        }
        count += 1;
        let label = op["id"].as_str().unwrap();
        let v = &row["value"];
        assert_eq!(v["success"], true, "{label}");
        let m = morph(&op).unwrap();
        let sources = op["points"].as_array().unwrap();
        let after = v["after"].as_array().unwrap();
        let pose = (op["rigid"] == true).then(|| m.rigid_transform(point(3.5, 1., 2.5)).unwrap());
        assert_eq!(
            after.len(),
            sources.len() * if op["copy"] == true { 2 } else { 1 }
        );
        for (src, dst) in sources.iter().zip(&after[after.len() - sources.len()..]) {
            let actual = pose.map_or_else(
                || m.morph_point(p(src)).unwrap(),
                |pose| pose.transform_point(p(src)).unwrap(),
            );
            near(
                actual,
                p(&dst["point"]),
                if pose.is_some() { 1e-7 } else { 1e-11 },
                0.,
                label,
            );
        }
        if !v["undo"].is_null() {
            assert_eq!(v["undo"], v["before"], "{label}");
            assert_eq!(v["redo"], v["after"], "{label}");
        }
    }
    assert_eq!(count, 8);
}

fn world() -> Frame3 {
    Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn maelstrom_world_axis_rotations_match_independent_rodrigues_maps() {
    for index in 0..3 {
        for sign in [-1., 1.] {
            let mut n = [0.; 3];
            n[index] = sign;
            let normal = Vector3::try_from(n).unwrap().normalized_nonzero().unwrap();
            let frame =
                Frame3::try_from_normal(point(0., 0., 0.), normal.as_vector(), Tolerance::DEFAULT)
                    .unwrap();
            let source = point(6., 7., 8.);
            for angle in [
                -1.,
                1e-14,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            ] {
                let m = MaelstromPointMorph::try_new(frame, 2., 5., angle).unwrap();
                let expected = AffineTransform3::try_rotation(frame.origin(), normal, angle)
                    .unwrap()
                    .transform_point(source)
                    .unwrap();
                near(
                    m.morph_point(source).unwrap(),
                    expected,
                    2e-14,
                    0.,
                    "signed world axis",
                );
            }
        }
    }
}

#[test]
fn maelstrom_preserves_tiny_angles_cardinal_residuals_and_representable_extreme_images() {
    let tiny = MaelstromPointMorph::try_new(world(), 2., 5., 1e-14).unwrap();
    let actual = tiny.morph_point(point(6., 0., 7.)).unwrap();
    assert_eq!(actual.x(), 6.);
    assert_eq!(actual.y(), 6e-14);
    assert_eq!(actual.z(), 7.);
    let quarter =
        MaelstromPointMorph::try_new(world(), 2., 5., std::f64::consts::FRAC_PI_2).unwrap();
    let expected = point(6. * std::f64::consts::FRAC_PI_2.cos(), 6., 7.);
    near(
        quarter.morph_point(point(6., 0., 7.)).unwrap(),
        expected,
        1e-28,
        0.,
        "raw cardinal residual",
    );
    let frame = world().with_origin(point(0., 0., -Real::MAX));
    let m = MaelstromPointMorph::try_new(frame, 2., 5., 1e-14).unwrap();
    let actual = m.morph_point(point(6., 0., Real::MAX)).unwrap();
    assert_eq!(actual, point(6., 6e-14, Real::MAX));
    let frame = world().with_origin(point(-Real::MAX, 0., 0.));
    let m = MaelstromPointMorph::try_new(frame, 1e308, 1e308, 1e-300).unwrap();
    let source = point(Real::MAX, 0., 0.);
    let angle = m.angle_at(source).unwrap();
    let actual = m.morph_point(source).unwrap();
    let expected_y = crate::exact_scalar::scalar(
        &(crate::exact_scalar::rational(2.)
            * crate::exact_scalar::rational(Real::MAX)
            * crate::exact_scalar::rational(angle.sin())),
    )
    .unwrap();
    assert_eq!(actual.x(), Real::MAX);
    assert_eq!(actual.y(), expected_y);
    let fixed = MaelstromPointMorph::try_new(frame, 5., 2., 1.).unwrap();
    assert_eq!(fixed.morph_point(source).unwrap(), source);
    assert!(
        MaelstromPointMorph::try_new(frame, 2., 5., std::f64::consts::PI)
            .unwrap()
            .morph_point(source)
            .is_err()
    );
    assert!(
        MaelstromPointMorph::try_new(world(), 2., 2., Real::MAX)
            .unwrap()
            .morph_point(point(Real::MAX, 0., 0.))
            .is_err()
    );
    for bad in [Real::NAN, Real::INFINITY, -1., 0., 2f64.powi(-32)] {
        assert!(MaelstromPointMorph::try_new(world(), bad, 5., 1.).is_err());
    }
}

#[test]
fn maelstrom_control_preservation_identity_and_fitted_lines_keep_their_contracts() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(point(3., 1., 0.), 0.75).unwrap(),
            WeightedPoint3::try_new(point(6., 0., 0.), 1.).unwrap(),
        ],
        vec![2., 2., 2., 8., 8., 8.],
    )
    .unwrap();
    let m = MaelstromPointMorph::try_new(world(), 2., 5., 0.5).unwrap();
    let preserved = m
        .with_preserve_structure(true)
        .morph_nurbs_curve(&curve, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(preserved.degree(), curve.degree());
    assert_eq!(preserved.knots(), curve.knots());
    for (a, b) in preserved
        .control_points()
        .iter()
        .zip(curve.control_points())
    {
        assert_eq!(a.weight(), b.weight());
        assert_eq!(a.point(), m.morph_point(b.point()).unwrap());
    }
    let identity = MaelstromPointMorph::try_new(world(), 2., 5., 0.).unwrap();
    assert_eq!(
        identity
            .morph_nurbs_curve(&curve, Tolerance::DEFAULT)
            .unwrap(),
        curve
    );
    let tolerance = Tolerance::try_new(1e-5, 1e-12, 1e-9).unwrap();
    let line = LineSegment::try_new(point(1., 0., 0.), point(6., 0., 0.), tolerance).unwrap();
    let fitted = m.morph_line(line, tolerance).unwrap();
    let d = fitted.domain();
    for i in 0..=512 {
        let t = i as Real / 512.;
        let source = line.point_at(t).unwrap();
        near(
            fitted
                .evaluate(d.start() + (d.end() - d.start()) * t)
                .unwrap(),
            m.morph_point(source).unwrap(),
            2e-5,
            0.,
            "fitted line",
        );
    }
}
