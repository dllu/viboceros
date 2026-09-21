use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind};

#[test]
fn composite_features_replay_eleven_captures_and_retain_uncaptured_center_discrepancy() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/composite_feature_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/composite_feature_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 12);
    assert_eq!(observed["results"].as_array().unwrap().len(), 12);
    let mut matched = 0;
    for (operation, record) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(id, record["id"].as_str().unwrap());
        let [SplitEdgeInput::Pick(pick)] = fixture.inputs.as_ref().unwrap().as_slice() else {
            panic!()
        };
        let target = Point3::try_from(pick.point).unwrap();
        let mut document = Document::default();
        let object = document
            .add_geometry(
                fixture.base.sources[1]
                    .geometry(Tolerance::DEFAULT)
                    .unwrap(),
            )
            .unwrap();
        let snap = ObjectSnapCache::default()
            .nearest_projected(&document, [target.x(), target.y()], 1e-5, |point| {
                Some([point.x(), point.y()])
            })
            .unwrap();
        let (actual, _) = run_split(fixture, Tolerance::DEFAULT).unwrap();
        if id == "polycurve-arc-center" {
            // The click near the empty center did not capture Cen. The adapter
            // replays a declared model target, so it must disagree here. Keep
            // this raw screen result rather than claiming a twelfth match.
            assert!(snap.is_none());
            let raw = record["value"]["after"][0]["geometry"]["brep"]["vertices"][8][0]
                .as_f64()
                .unwrap();
            assert!((raw - 2.776693248934896).abs() < 1e-12);
            assert_eq!(
                actual["after"][0]["geometry"]["brep"]["vertices"][8][0],
                json!(4.)
            );
            continue;
        }
        let snap = snap.unwrap();
        assert_eq!(snap.object_id(), object);
        assert_eq!(
            snap.kind(),
            match pick.osnap {
                SplitEdgeSnap::Mid => ObjectSnapKind::Mid,
                SplitEdgeSnap::End => ObjectSnapKind::End,
                _ => panic!(),
            }
        );
        assert!(snap.point().distance_to(target).unwrap() < 1e-10);
        let mut reference = record["value"].clone();
        for key in [
            "command_events",
            "command_history",
            "undo_events",
            "redo_events",
            "undo_event_snapshot",
            "redo_event_snapshot",
        ] {
            reference.as_object_mut().unwrap().remove(key);
        }
        close(&actual, &reference, id);
        matched += 1;
    }
    assert_eq!(matched, 11);
}
