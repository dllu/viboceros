//! Default axes use whole-source planarity and the existing frame/basis model.
use super::automatic_tests::{add_line, native_surface};
use super::curved_tests::rail;
use super::*;
use serde_json::Value;

#[test]
fn roadlike_default_matches_complete_explicit_axis_command_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let input: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/sweep1_roadlike_explicit.json")).unwrap(),
    )
    .unwrap();
    let observed: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/sweep1_roadlike_explicit.json")).unwrap(),
    )
    .unwrap();
    let mut checked = 0;
    for (op, row) in input["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let tol = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
        let mut doc = Document::new(tol);
        let registry = CommandRegistry::with_builtins();
        let rail = rail(&mut doc, &op["rail"]);
        let profile = add_line(&mut doc, &op["sections"][0]);
        doc.select_objects_direct([rail, profile], SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut doc,
                &format!(
                    "Sweep1 RailId={rail} Parameters=0 FrameStyle=Roadlike RefitRail={}",
                    if op["refit_rail"] == true {
                        "Yes"
                    } else {
                        "No"
                    }
                ),
            )
            .unwrap();
        for source in &before {
            assert_eq!(doc.object(source.id()), Some(source));
        }
        let actual = doc
            .objects()
            .find_map(|o| match o.geometry() {
                Geometry::Brep(b) => Some(b),
                _ => None,
            })
            .unwrap();
        assert_eq!(row["value"].as_array().unwrap().len(), 1);
        let expected = native_surface(&row["value"][0]["definition"]);
        let atlas = actual
            .faces()
            .iter()
            .map(|f| f.surface())
            .collect::<Vec<_>>();
        // Refitting has a separate comparison budget from construction tolerance.
        // This check does not claim agreement within 1e-7 for the refitted arcs.
        let epsilon = if op["refit_rail"] == true { 1e-6 } else { 1e-7 };
        let bound = super::arc_station_tests::ruled_bound(&atlas, &[&expected], epsilon)
            .unwrap_or_else(|| panic!("whole Roadlike image differs: {}", op["id"]));
        println!(
            "ROADLIKE_RESULT {}",
            serde_json::json!({"id":op["id"],"complete_image_bound":bound,"comparison_epsilon":epsilon,"construction_tolerance":1e-7})
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
fn explicit_axis_override_keeps_the_parallel_axis_error_and_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let source = LineSegment::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(0., 0., 5.).unwrap(),
        doc.tolerance(),
    )
    .unwrap();
    let rail = doc.add_geometry(Geometry::Line(source)).unwrap();
    registry.execute(&mut doc, "Line 0,0,0 1,0,0").unwrap();
    let profile = doc.objects().last().unwrap().id();
    doc.select_objects_direct([rail, profile], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("Sweep1 RailId={rail} Parameters=0 FrameStyle=Roadlike Axis=0,0,1")
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry
        .execute(
            &mut doc,
            &format!("Sweep1 RailId={rail} Parameters=0 FrameStyle=Roadlike"),
        )
        .unwrap();
}
