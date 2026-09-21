use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn calibrated_center_hover_matches_fourteen_captures_and_eight_empty_center_misses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/center_hover_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/center_hover_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 22);
    assert_eq!(observed["results"].as_array().unwrap().len(), 22);
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
        let matrix: [[f64; 4]; 4] =
            serde_json::from_value(frame["world_to_screen"].clone()).unwrap();
        let cursor: [f64; 2] = serde_json::from_value(frame["click_client"].clone()).unwrap();
        let camera: [f64; 3] = serde_json::from_value(frame["camera_location"].clone()).unwrap();
        let direction: [f64; 3] =
            serde_json::from_value(frame["camera_direction"].clone()).unwrap();
        let project = |point: Point3| {
            let p = point.to_array();
            if (0..3)
                .map(|i| (p[i] - camera[i]) * direction[i])
                .sum::<f64>()
                <= 0.
            {
                return None;
            }
            let h: [f64; 4] = std::array::from_fn(|i| {
                matrix[i][3] + (0..3).map(|j| matrix[i][j] * p[j]).sum::<f64>()
            });
            if h[3] == 0. {
                None
            } else {
                Some([h[0] / h[3], h[1] / h[3]])
            }
        };
        let aim = Point3::try_from(pick.aim.unwrap()).unwrap();
        let expected_pixel: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        let actual_pixel = project(aim).unwrap();
        for i in 0..2 {
            assert!((actual_pixel[i] - expected_pixel[i]).abs() < 1e-7);
        }
        let modes = if pick.osnap == SplitEdgeSnap::Cen {
            ObjectSnapModes::only(ObjectSnapKind::Center)
        } else {
            assert_eq!(pick.osnap, SplitEdgeSnap::Persistent);
            assert_eq!(fixture.persistent_snaps.len(), 5);
            ObjectSnapModes::ALL
        };
        let mut doc = Document::default();
        let objects: Vec<_> = fixture
            .base
            .sources
            .iter()
            .map(|source| {
                doc.add_geometry(source.geometry(Tolerance::DEFAULT).unwrap())
                    .unwrap()
            })
            .collect();
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(&doc, cursor, 12., project, modes)
            .unwrap();
        if id.ends_with("empty-center") {
            assert!(snap.is_none(), "{id}: {snap:?}");
            // Admission is tested against the actual pixels. The remaining
            // unconstrained screen-to-edge parameter is not replayed here.
            let raw = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
                .as_f64()
                .unwrap();
            assert!((raw - 4.).abs() > 1.);
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing snap"));
        assert_eq!(snap.object_id(), objects[1], "{id}");
        let kind = if id.ends_with("near-end") {
            ObjectSnapKind::End
        } else if id.ends_with("near-mid") {
            ObjectSnapKind::Mid
        } else if id.ends_with("near-quad") {
            ObjectSnapKind::Quad
        } else {
            ObjectSnapKind::Center
        };
        assert_eq!(snap.kind(), kind, "{id}");
        if matches!(
            id.as_str(),
            "circle-all-near-point" | "circle-all-other-point"
        ) {
            // The original request hypothesized the point object would win.
            // Keep that input untouched: both engines actually capture Center.
            assert_eq!(snap.point(), Point3::try_from([4., -4., 0.]).unwrap());
            assert_ne!(snap.point().to_array(), pick.point);
        } else {
            assert!(
                snap.point()
                    .distance_to(Point3::try_from(pick.point).unwrap())
                    .unwrap()
                    < 1e-10,
                "{id}"
            );
        }
        // Feed the production capture result into the command, without reading
        // Rhino's result positions or replacing the retained request hypothesis.
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(input) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        input.point = snap.point().to_array();
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        let mut expected = value.clone();
        for key in [
            "pick_frames",
            "command_events",
            "command_history",
            "undo_events",
            "redo_events",
            "undo_event_snapshot",
            "redo_event_snapshot",
        ] {
            expected.as_object_mut().unwrap().remove(key);
        }
        close(&actual, &expected, id);
        captures += 1;
    }
    assert_eq!((captures, misses), (14, 8));
}
