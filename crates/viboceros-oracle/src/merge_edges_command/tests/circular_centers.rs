use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn circular_nurbs_and_edges_replay_thirty_four_captures_with_explicit_misses_and_limits() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/circular_center_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/circular_center_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 44);
    assert_eq!(observed["results"].as_array().unwrap().len(), 44);
    let (mut matches, mut misses, mut negative_gauges, mut elliptic_limits) = (0, 0, 0, 0);
    for (operation, row) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(id, row["id"].as_str().unwrap());
        let [SplitEdgeInput::Pick(pick)] = fixture.inputs.as_ref().unwrap().as_slice() else {
            panic!()
        };
        let value = &row["value"];
        let [frame] = value["pick_frames"].as_array().unwrap().as_slice() else {
            panic!()
        };
        let project = calibrated_snap::projection(frame);
        let pixel = project(Point3::try_from(pick.aim.unwrap()).unwrap()).unwrap();
        let expected_pixel: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        for i in 0..2 {
            assert!((pixel[i] - expected_pixel[i]).abs() < 1e-7);
        }
        if pick.osnap == SplitEdgeSnap::Persistent {
            assert_eq!(fixture.persistent_snaps, [SplitEdgeSnap::Cen]);
        } else {
            assert_eq!(pick.osnap, SplitEdgeSnap::Cen);
        }
        let mut document = Document::default();
        let objects: Vec<_> = fixture
            .base
            .sources
            .iter()
            .map(|s| {
                document
                    .add_geometry(s.geometry(Tolerance::DEFAULT).unwrap())
                    .unwrap()
            })
            .collect();
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(
                &document,
                serde_json::from_value(frame["click_client"].clone()).unwrap(),
                12.,
                project,
                ObjectSnapModes::only(ObjectSnapKind::Center),
            )
            .unwrap();
        if id.starts_with("outside-arc-") {
            assert!(snap.is_none(), "{id}: {snap:?}");
            assert_eq!(value["succeeded"], false);
            assert_eq!(value["history_tested"], false);
            assert_eq!(value["before"], value["after"]);
            misses += 1;
            continue;
        }
        assert_eq!(value["succeeded"], true, "{id}");
        assert_eq!(value["history_tested"], true, "{id}");
        let observed_x = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
            .as_f64()
            .unwrap();
        if id.starts_with("empty-center-") {
            assert!(snap.is_none(), "{id}: {snap:?}");
            assert!((observed_x - 4.).abs() > 0.5);
            misses += 1;
            continue;
        }
        if id.starts_with("ellipse-") || id.starts_with("noncircle-") {
            assert!(snap.is_none(), "{id}: {snap:?}");
            let expected = if id.starts_with("ellipse-") { 4. } else { 3.75 };
            assert!((observed_x - expected).abs() < 1e-9);
            // Retain the raw ellipse captures as known unsupported behavior,
            // not as admission misses or passing command geometry comparisons.
            elliptic_limits += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing Center"));
        assert_eq!(snap.kind(), ObjectSnapKind::Center);
        assert_eq!(snap.object_id(), objects[1], "{id}");
        // A projected annulus/cylinder can have multiple coincident centers;
        // the scalar constrained result establishes x, not an occlusion policy.
        if id.starts_with("extruded-") {
            assert!((snap.point().x() - 4.).abs() < 1e-9 && (snap.point().y() + 4.).abs() < 1e-9);
        } else {
            assert!(
                snap.point()
                    .distance_to(Point3::try_from(pick.point).unwrap())
                    .unwrap()
                    < 1e-9,
                "{id}: {snap:?}"
            );
        }
        if id.starts_with("quarter-negative-") {
            assert!((observed_x - snap.point().x()).abs() > 0.5);
            // A common negative homogeneous gauge leaves geometry unchanged;
            // do not reproduce Rhino's measured recognition failure.
            negative_gauges += 1;
            continue;
        }
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(pick) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        pick.point = snap.point().to_array();
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(value), id);
        matches += 1;
    }
    assert_eq!(
        (matches, misses, negative_gauges, elliptic_limits),
        (34, 4, 2, 4)
    );
}
