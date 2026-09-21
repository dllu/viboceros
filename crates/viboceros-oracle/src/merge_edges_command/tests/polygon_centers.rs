use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn calibrated_polygon_centers_match_thirty_four_captures_and_ten_misses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/polygon_center_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/polygon_center_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 44);
    assert_eq!(observed["results"].as_array().unwrap().len(), 44);
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
        let cursor = serde_json::from_value(frame["click_client"].clone()).unwrap();
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(
                &document,
                cursor,
                12.,
                project,
                ObjectSnapModes::only(ObjectSnapKind::Center),
            )
            .unwrap();
        let miss = [
            "open-",
            "near-closed-",
            "empty-center-",
            "surface-interior-",
            "warped-surface-",
        ]
        .iter()
        .any(|prefix| id.starts_with(prefix));
        assert_eq!(value["succeeded"], true, "{id}");
        assert_eq!(value["history_tested"], true, "{id}");
        if miss {
            assert!(snap.is_none(), "{id}: {snap:?}");
            let actual_x = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
                .as_f64()
                .unwrap();
            assert!((actual_x - pick.point[0]).abs() > 0.5, "{id}");
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing Center"));
        assert_eq!(snap.kind(), ObjectSnapKind::Center);
        assert_eq!(snap.object_id(), objects[1], "{id}");
        // The original subdivided-surface hypothesis counts parameterization
        // vertices; Rhino instead uses the four actual boundary corners.
        let target = if id.starts_with("subdivided-surface-") {
            [4.75, -4.25, 0.]
        } else {
            pick.point
        };
        assert!(
            snap.point()
                .distance_to(Point3::try_from(target).unwrap())
                .unwrap()
                < 1e-9,
            "{id}: {snap:?}"
        );
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(pick) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        pick.point = snap.point().to_array(); // Never feed recorded Rhino output into replay.
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(value), id);
        captures += 1;
    }
    assert_eq!((captures, misses), (34, 10));
}
