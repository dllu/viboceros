use super::*;
use crate::SelectionMode;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 2.0 * x, 3.0 * x).unwrap())
}

#[test]
fn unit_changes_match_rhino8_public_api_measurements() {
    use serde_json::Value;
    fn units(code: &Value) -> LengthUnitSystem {
        match code.as_u64().unwrap() {
            0 => LengthUnitSystem::None,
            2 => LengthUnitSystem::Millimeters,
            4 => LengthUnitSystem::Meters,
            8 => LengthUnitSystem::Inches,
            _ => panic!("unexpected fixture unit"),
        }
    }
    fn check(document: &Document, state: &Value) {
        assert_eq!(document.units(), &units(&state["units"]));
        for (actual, key) in [
            (document.tolerance.absolute(), "absolute"),
            (document.tolerance.relative(), "relative"),
            (document.tolerance.angular(), "angular"),
        ] {
            assert_eq!(actual, state[key].as_f64().unwrap());
        }
        let expected = state["objects"].as_array().unwrap();
        assert_eq!(document.objects.len(), expected.len());
        for (object, expected) in document.objects.iter().zip(expected) {
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected point");
            };
            for (actual, expected) in [point.x(), point.y(), point.z()]
                .into_iter()
                .zip(expected["point"].as_array().unwrap())
            {
                let expected = expected.as_f64().unwrap();
                assert!((actual - expected).abs() <= 1e-12 * expected.abs().max(1.0));
            }
            assert_eq!(
                object.attributes().is_visible(),
                expected["mode"] != "Hidden"
            );
            assert_eq!(
                object.attributes().is_locked(),
                expected["mode"] == "Locked"
            );
            assert_eq!(
                document.selection.contains(&object.id),
                expected["selected"].as_bool().unwrap()
            );
        }
    }
    let fixture: Value = serde_json::from_str(include_str!("fixtures/rhino8.json")).unwrap();
    assert_eq!(fixture["engine"], "rhino");
    assert!(fixture.get("error").is_none());
    let cases = fixture["results"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let before = &case["value"]["before"];
        let after = &case["value"]["after"];
        let mut document = Document::with_units(
            Tolerance::try_new(0.001, 0.0001, 0.00001).unwrap(),
            units(&before["units"]),
        )
        .unwrap();
        let first = document.add_geometry(point(1000.0)).unwrap();
        let hidden = document.add_geometry(point(500.0)).unwrap();
        let locked = document.add_geometry(point(250.0)).unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        document.set_objects_visibility([hidden], false).unwrap();
        document.set_objects_locked([locked], true).unwrap();
        check(&document, before);
        let rescale = case["id"].as_str().unwrap().ends_with("-true");
        document.set_units(units(&after["units"]), rescale).unwrap();
        check(&document, after);
        document.undo().unwrap();
        check(&document, before);
        document.redo().unwrap();
        check(&document, after);
    }
}

#[test]
fn unit_scaling_includes_hidden_locked_objects_and_replays_exactly() {
    let mut document = Document::new(Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap());
    let first = document.add_geometry(point(1000.0)).unwrap();
    let second = document.add_geometry(point(500.0)).unwrap();
    let hidden = document.add_geometry(point(250.0)).unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();
    document.set_objects_visibility([hidden], false).unwrap();
    document.set_objects_locked([second], true).unwrap();
    document
        .add_group(Some("parts".into()), [first, second, hidden])
        .unwrap();
    let objects = document.objects.clone();
    let selection = document.selection.clone();
    let groups = document.groups.clone();
    let tolerance = document.tolerance;
    assert!(document.set_units(LengthUnitSystem::Meters, true).unwrap());
    assert_eq!(document.object(first).unwrap().geometry(), &point(1.0));
    assert_eq!(document.object(second).unwrap().geometry(), &point(0.5));
    assert_eq!(document.object(hidden).unwrap().geometry(), &point(0.25));
    assert!(document.object(second).unwrap().attributes().is_locked());
    assert!(!document.object(hidden).unwrap().attributes().is_visible());
    assert_eq!(document.selection, selection);
    assert_eq!(document.groups, groups);
    assert_eq!(document.tolerance.absolute(), tolerance.absolute());
    assert_eq!(document.tolerance.relative(), tolerance.relative());
    assert_eq!(document.tolerance.angular(), tolerance.angular());
    assert_eq!(document.undo_label(), Some("Units"));
    let scaled = document.objects.clone();
    document.undo().unwrap();
    assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
    assert_eq!(document.objects, objects);
    assert_eq!(document.tolerance, tolerance);
    document.redo().unwrap();
    assert_eq!(document.objects, scaled);
    assert_eq!(document.units(), &LengthUnitSystem::Meters);
    assert_eq!(document.selection, selection);
}

#[test]
fn metadata_only_units_preserve_geometry_tolerance_and_noop_redo() {
    let mut document = Document::default();
    document.add_geometry(point(10.0)).unwrap();
    let objects = document.objects.clone();
    let tolerance = document.tolerance;
    document.set_units(LengthUnitSystem::Inches, false).unwrap();
    assert_eq!(document.objects, objects);
    assert_eq!(document.tolerance, tolerance);
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert!(
        !document
            .set_units(LengthUnitSystem::Millimeters, true)
            .unwrap()
    );
    assert_eq!(format!("{document:?}"), before);
    document.redo().unwrap();
    assert_eq!(document.units(), &LengthUnitSystem::Inches);
    assert_eq!(document.objects, objects);
}

#[test]
fn unit_changes_roll_back_with_interleaved_object_edits() {
    let mut document = Document::default();
    let original = document.add_geometry(point(1000.0)).unwrap();
    document.add_geometry(point(2000.0)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    document.begin_transaction("mixed units").unwrap();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    document.delete_object(original).unwrap();
    document.add_geometry(point(3.0)).unwrap();
    document
        .set_units(LengthUnitSystem::Centimeters, true)
        .unwrap();
    document.rollback_transaction().unwrap();
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn invalid_units_and_conversion_failures_do_not_mutate_any_document_state() {
    let mut document = Document::default();
    // This object can be converted even when the following object overflows.
    document.add_geometry(point(1.0)).unwrap();
    document.add_geometry(point(1e100)).unwrap();
    let before = format!("{document:?}");
    for (units, rescale) in [
        (
            LengthUnitSystem::Custom {
                name: "invalid".into(),
                meters_per_unit: f64::NAN,
            },
            false,
        ),
        (LengthUnitSystem::Unset, true),
        (
            LengthUnitSystem::Custom {
                name: "tiny".into(),
                meters_per_unit: 1e-300,
            },
            true,
        ),
    ] {
        assert!(document.set_units(units, rescale).is_err());
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn repeated_unit_history_replay_does_not_accumulate_roundoff() {
    let mut document = Document::default();
    // Multiplying 1.3 by .001 and then 1000 does not recover its exact bits.
    document.add_geometry(point(1.3)).unwrap();
    let original = document.objects.clone();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    let converted = document.objects.clone();
    for _ in 0..32 {
        document.undo().unwrap();
        assert_eq!(document.objects, original);
        assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
        document.redo().unwrap();
        assert_eq!(document.objects, converted);
        assert_eq!(document.units(), &LengthUnitSystem::Meters);
    }
}
