use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn calibrated_mid_hover_matches_twenty_six_captures_and_eleven_mixed_mode_misses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/mid_hover_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mid_hover_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 37);
    assert_eq!(observed["results"].as_array().unwrap().len(), 37);
    let (mut captures, mut misses) = (0, 0);
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
        let aim = Point3::try_from(pick.aim.unwrap()).unwrap();
        let pixel = project(aim).unwrap();
        let expected_pixel: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        for i in 0..2 {
            assert!((pixel[i] - expected_pixel[i]).abs() < 1e-7);
        }
        let modes = if pick.osnap == SplitEdgeSnap::Mid {
            ObjectSnapModes::only(ObjectSnapKind::Mid)
        } else {
            assert_eq!(pick.osnap, SplitEdgeSnap::Persistent);
            fixture
                .persistent_snaps
                .iter()
                .fold(ObjectSnapModes::NONE, |set, &mode| {
                    set.with(
                        match mode {
                            SplitEdgeSnap::Point => ObjectSnapKind::Point,
                            SplitEdgeSnap::End => ObjectSnapKind::End,
                            SplitEdgeSnap::Mid => ObjectSnapKind::Mid,
                            SplitEdgeSnap::Cen => ObjectSnapKind::Center,
                            SplitEdgeSnap::Quad => ObjectSnapKind::Quad,
                            _ => panic!(),
                        },
                        true,
                    )
                })
        };
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
        let cursor = serde_json::from_value(frame["click_client"].clone()).unwrap();
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(&document, cursor, 12., project, modes)
            .unwrap();
        if id.ends_with("-mixed") || id.ends_with("-mixed-hover") {
            assert!(snap.is_none(), "{id}: {snap:?}");
            // Only admission is replayed for unsnapped screen-to-edge inputs.
            // The rational control's unconstrained click fails to split; retain it.
            if id == "rational-mixed" {
                assert_eq!(value["succeeded"], false);
            } else {
                assert_eq!(value["succeeded"], true);
                let raw = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
                    .as_f64()
                    .unwrap();
                assert!((raw - pick.point[0]).abs() > 0.5, "{id}");
            }
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing Mid"));
        if id.ends_with("all-target") {
            // Here Mid and Quad coincide; geometry cannot distinguish the label.
            assert!(matches!(
                snap.kind(),
                ObjectSnapKind::Mid | ObjectSnapKind::Quad
            ));
        } else {
            assert_eq!(snap.kind(), ObjectSnapKind::Mid, "{id}");
        }
        assert_eq!(
            snap.object_id(),
            objects[usize::from(!id.starts_with("brep-edge"))],
            "{id}"
        );
        assert!(
            snap.point()
                .distance_to(Point3::try_from(pick.point).unwrap())
                .unwrap()
                < 1e-9,
            "{id}: {snap:?}"
        );
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(target) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        target.point = snap.point().to_array();
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(value), id);
        captures += 1;
    }
    assert_eq!((captures, misses), (26, 11));
}
