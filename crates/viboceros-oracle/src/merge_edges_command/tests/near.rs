use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn calibrated_near_targets_replay_all_sixteen_complete_rhino_histories() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/near_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/near_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 16);
    for (operation, row) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(id, row["id"].as_str().unwrap());
        assert!(
            run_split(fixture, Tolerance::DEFAULT).is_err(),
            "raw aims are not targets"
        );
        let mut doc = Document::default();
        let objects: Vec<_> = fixture
            .base
            .sources
            .iter()
            .map(|s| {
                doc.add_geometry(s.geometry(Tolerance::DEFAULT).unwrap())
                    .unwrap()
            })
            .collect();
        let kind = |mode| match mode {
            SplitEdgeSnap::Point => ObjectSnapKind::Point,
            SplitEdgeSnap::End => ObjectSnapKind::End,
            SplitEdgeSnap::Mid => ObjectSnapKind::Mid,
            SplitEdgeSnap::Cen => ObjectSnapKind::Center,
            SplitEdgeSnap::Quad => ObjectSnapKind::Quad,
            SplitEdgeSnap::Near => ObjectSnapKind::Near,
            _ => panic!(),
        };
        let SplitEdgeInput::Pick(pick) = &fixture.inputs.as_ref().unwrap()[0] else {
            panic!()
        };
        let modes = if pick.osnap == SplitEdgeSnap::Persistent {
            fixture
                .persistent_snaps
                .iter()
                .fold(ObjectSnapModes::NONE, |m, &s| m.with(kind(s), true))
        } else {
            ObjectSnapModes::only(kind(pick.osnap))
        };
        let frame = &row["value"]["pick_frames"][0];
        let project = calibrated_snap::projection(frame);
        let aim = project(Point3::try_from(pick.aim.unwrap()).unwrap()).unwrap();
        let pixels: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        for i in 0..2 {
            assert!((aim[i] - pixels[i]).abs() < 1e-7);
        }
        let cursor = serde_json::from_value(frame["click_client"].clone()).unwrap();
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(&doc, cursor, 12., project, modes)
            .unwrap()
            .unwrap_or_else(|| panic!("{id}: no capture"));
        assert_eq!(snap.object_id(), objects[1], "{id}");
        let expected_kind = match id.as_str() {
            "line-end" => ObjectSnapKind::End,
            "line-mid" => ObjectSnapKind::Mid,
            "circle-quad" => ObjectSnapKind::Quad,
            _ => ObjectSnapKind::Near,
        };
        assert_eq!(snap.kind(), expected_kind, "{id}");
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(target) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        target.point = snap.point().to_array();
        // The command adapter consumes resolved model points. Near itself is
        // evaluated above from source/camera/click, never from result positions.
        target.osnap = SplitEdgeSnap::Point;
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(&row["value"]), id);
    }
}
