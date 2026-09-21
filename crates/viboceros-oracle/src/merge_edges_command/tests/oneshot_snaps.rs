use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

fn kind(mode: SplitEdgeSnap) -> ObjectSnapKind {
    match mode {
        SplitEdgeSnap::Point => ObjectSnapKind::Point,
        SplitEdgeSnap::End => ObjectSnapKind::End,
        SplitEdgeSnap::Mid => ObjectSnapKind::Mid,
        SplitEdgeSnap::Cen => ObjectSnapKind::Center,
        SplitEdgeSnap::Quad => ObjectSnapKind::Quad,
        _ => panic!("not a feature"),
    }
}

#[test]
fn calibrated_one_shot_captures_then_restored_persistent_modes_replay_complete_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/oneshot_snap_modes.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/oneshot_snap_modes.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 4);
    assert_eq!(observed["results"].as_array().unwrap().len(), 4);
    let mut count = 0;
    for (operation, row) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(id, row["id"].as_str().unwrap());
        let value = &row["value"];
        let frames = value["pick_frames"].as_array().unwrap();
        let steps = fixture.inputs.as_ref().unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(steps.len(), 2);
        let mut document = Document::default();
        let objects: Vec<_> = fixture
            .base
            .sources
            .iter()
            .map(|source| {
                document
                    .add_geometry(source.geometry(Tolerance::DEFAULT).unwrap())
                    .unwrap()
            })
            .collect();
        let persistent = fixture
            .persistent_snaps
            .iter()
            .fold(ObjectSnapModes::NONE, |set, &mode| {
                set.with(kind(mode), true)
            });
        let mut captured = fixture.clone();
        let mut cache = ObjectSnapCache::default();
        for (index, (input, frame)) in steps.iter().zip(frames).enumerate() {
            let SplitEdgeInput::Pick(pick) = input else {
                panic!()
            };
            let modes = if index == 0 {
                assert_ne!(pick.osnap, SplitEdgeSnap::Persistent);
                ObjectSnapModes::only(kind(pick.osnap))
            } else {
                assert_eq!(pick.osnap, SplitEdgeSnap::Persistent);
                persistent
            };
            let project = calibrated_snap::projection(frame);
            let pixel = project(Point3::try_from(pick.aim.unwrap()).unwrap()).unwrap();
            let expected_pixel: [f64; 2] =
                serde_json::from_value(frame["aim_client"].clone()).unwrap();
            for i in 0..2 {
                assert!((pixel[i] - expected_pixel[i]).abs() < 1e-7);
            }
            let cursor = serde_json::from_value(frame["click_client"].clone()).unwrap();
            let snap = cache
                .nearest_projected_with_modes(&document, cursor, 12., project, modes)
                .unwrap()
                .unwrap_or_else(|| panic!("{id}/{index}: missing snap"));
            let expected_kind = if index == 0 {
                kind(pick.osnap)
            } else if id.ends_with("all") {
                ObjectSnapKind::Quad
            } else {
                ObjectSnapKind::Center
            };
            assert_eq!(snap.kind(), expected_kind, "{id}/{index}");
            let owner = if matches!(expected_kind, ObjectSnapKind::Point | ObjectSnapKind::Mid) {
                2
            } else {
                1
            };
            assert_eq!(snap.object_id(), objects[owner], "{id}/{index}");
            assert!(
                snap.point()
                    .distance_to(Point3::try_from(pick.point).unwrap())
                    .unwrap()
                    < 1e-10,
                "{id}/{index}"
            );
            let SplitEdgeInput::Pick(target) = &mut captured.inputs.as_mut().unwrap()[index] else {
                panic!()
            };
            target.point = snap.point().to_array();
            count += 1;
        }
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(value), id);
    }
    assert_eq!(count, 8);
}
