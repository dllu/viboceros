use super::*;
use serde_json::Value;

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn near(a: Point3, b: Point3, label: &str) {
    assert!(a.distance_to(b).unwrap() < 1e-11, "{label}: {a:?} != {b:?}");
}

fn xy() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_from([0., 0., 0.]).unwrap(),
        viboceros_geometry::Vector3::try_from([0., 0., 1.]).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}

#[test]
fn circle_definitions_match_owned_native_circle_getters_and_morph_points() {
    for (fixture, observed) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_circle.json"),
            include_str!("../../../../tools/rhino_oracle/observations/maelstrom_circle.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_circle_point.json"),
            include_str!("../../../../tools/rhino_oracle/observations/maelstrom_circle_point.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_circle_angle.json"),
            include_str!("../../../../tools/rhino_oracle/observations/maelstrom_circle_angle.json"),
        ),
    ] {
        compare_circles(fixture, observed);
    }
}

fn compare_circles(fixture: &str, observed: &str) {
    let fixture: Value = serde_json::from_str(fixture).unwrap();
    let observed: Value = serde_json::from_str(observed).unwrap();
    let mut diameter = false;
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let v = &row["value"];
        let label = op["id"].as_str().unwrap();
        let plane = Frame3::try_from_normal(
            p(&op["origin"]),
            viboceros_geometry::Vector3::try_from(p(&op["normal"]).to_array()).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let mut getter = CircleInput::new(plane).with_size_mode(if diameter {
            CircleSizeMode::Diameter
        } else {
            CircleSizeMode::Radius
        });
        diameter = v["diameter"].as_bool().unwrap();
        let mut circle = None;
        if v["circle"].is_null() {
            continue;
        }
        let inputs = v["resolved_inputs"].as_array().unwrap();
        for value in &inputs[..inputs.len() - 4] {
            if let Some(name) = value.as_str() {
                if name.starts_with("ProjectOsnap=") {
                    continue;
                }
                assert!(getter.option(name), "{label}: option {name}");
            } else if value.is_array() {
                circle = getter.point(p(value)).unwrap();
            } else {
                circle = getter.number(value.as_f64().unwrap()).unwrap();
            }
        }
        let circle = circle.unwrap();
        let native = &v["circle"];
        near(circle.frame.origin(), p(&native["origin"]), label);
        near(
            Point3::try_from(circle.frame.x_axis().as_vector().to_array()).unwrap(),
            p(&native["x"]),
            label,
        );
        near(
            Point3::try_from(circle.frame.y_axis().as_vector().to_array()).unwrap(),
            p(&native["y"]),
            label,
        );
        assert!(
            (circle.radius - native["radius"].as_f64().unwrap()).abs() < 1e-11,
            "{label}"
        );
        let degrees = if op["degrees"].is_array() {
            crate::maelstrom::coil_angle(circle.frame, p(&op["degrees"])).unwrap()
        } else {
            op["degrees"].as_f64().unwrap()
        };
        let morph = viboceros_geometry::MaelstromPointMorph::try_new(
            circle.frame,
            circle.radius,
            op["target"].as_f64().unwrap(),
            degrees.to_radians(),
        )
        .unwrap();
        use viboceros_geometry::PointMorph;
        for (src, expected) in v["before"]
            .as_array()
            .unwrap()
            .iter()
            .zip(v["sdk_points"].as_array().unwrap())
        {
            near(
                morph.morph_point(p(&src["point"])).unwrap(),
                p(expected),
                label,
            );
        }
    }
}

#[test]
fn rejected_geometry_retains_getter_and_next_valid_pick_succeeds() {
    let plane = xy();
    let a = Point3::try_from([0., 0., 0.]).unwrap();
    let b = Point3::try_from([4., 0., 0.]).unwrap();
    let c = Point3::try_from([2., 3., 0.]).unwrap();
    let mut getter = CircleInput::new(plane);
    assert!(getter.option("3Point"));
    getter.point(a).unwrap();
    let pending = getter;
    assert!(getter.point(a).is_err());
    assert_eq!(getter, pending);
    getter.point(b).unwrap();
    let pending = getter;
    assert!(
        getter
            .point(Point3::try_from([2., 0., 0.]).unwrap())
            .is_err()
    );
    assert_eq!(getter, pending);
    assert!(getter.point(c).unwrap().is_some());
    assert!(getter.option("Radius"));
    assert!(!getter.has_size_mode());
    let pending = getter;
    assert!(getter.number(1.).is_err());
    assert_eq!(getter, pending);
    assert_eq!(getter.number(3.).unwrap(), None);
    let pending = getter;
    assert!(getter.point(b).is_err());
    assert_eq!(getter, pending);
    assert!(getter.point(c).unwrap().is_some());

    let mut getter = CircleInput::at_center(plane, a);
    assert!(getter.option("Orientation"));
    let pending = getter;
    assert!(getter.point(a).is_err());
    assert_eq!(getter, pending);
    getter
        .point(Point3::try_from([0., 0., 1.]).unwrap())
        .unwrap();
    let pending = getter;
    assert!(
        getter
            .point(Point3::try_from([0., 0., 4.]).unwrap())
            .is_err()
    );
    assert_eq!(getter, pending);
    assert!(getter.point(c).unwrap().is_some());
}

#[test]
fn invalid_sizes_preserve_pending_input_and_preview_is_read_only() {
    let center = Point3::try_from([0., 0., 0.]).unwrap();
    let cursor = Point3::try_from([2., 1., 7.]).unwrap();
    for mode in [
        CircleSizeMode::Radius,
        CircleSizeMode::Diameter,
        CircleSizeMode::Circumference,
        CircleSizeMode::Area,
    ] {
        let mut getter = CircleInput::at_center(xy(), center).with_size_mode(mode);
        let pending = getter;
        for value in [0., -1., Real::NAN, Real::INFINITY] {
            assert!(getter.number(value).is_err(), "{mode:?}: {value}");
            assert_eq!(getter, pending);
        }
        let guide = getter.preview(cursor).unwrap();
        assert_eq!(getter, pending);
        assert_eq!(getter.point(cursor).unwrap(), Some(guide));
        assert!(getter.number(4.).unwrap().is_some());
    }
}

#[test]
fn diameter_circle_samples_match_native_rotated_and_original_cplanes() {
    for (fixture, observed) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/circle_two_point_rotated.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/circle_two_point_rotated.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/circle_two_point.json"),
            include_str!("../../../../docs/circle-two-point-rhino-reference.json"),
        ),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        let observed: Value = serde_json::from_str(observed).unwrap();
        for (op, row) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            let vector =
                |name| viboceros_geometry::Vector3::try_from(p(&op[name]).to_array()).unwrap();
            let plane = Frame3::try_from_directions(
                p(&op["origin"]),
                vector("x_axis"),
                vector("y_axis"),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            let circle = Circle3::try_from_diameter_on_plane(
                p(&op["points"][0]),
                p(&op["points"][1]),
                plane,
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            let samples = row["value"]["points"].as_array().unwrap();
            for (i, sample) in samples.iter().enumerate() {
                near(
                    circle
                        .point_at_angle(
                            std::f64::consts::TAU * i as Real / (samples.len() - 1) as Real,
                        )
                        .unwrap(),
                    p(sample),
                    op["id"].as_str().unwrap(),
                );
            }
        }
    }
}
