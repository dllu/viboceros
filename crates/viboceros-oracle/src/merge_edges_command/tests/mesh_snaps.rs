use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes, ObjectSnapOptions};

#[test]
fn calibrated_mesh_snaps_replay_seven_complete_rhino_histories() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/mesh_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_snaps.json"
    ))
    .unwrap();
    let (mut captures, mut misses) = (0, 0);
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/mesh_snap_weighted_targets.json"
    ))
    .unwrap();
    let screen_reference: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/mesh_snap_targets.json"
    ))
    .unwrap();
    let (mut mids, mut near) = (0, 0);
    for (operation, row) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(row["id"], *id);
        assert!(run_split(fixture, Tolerance::DEFAULT).is_err());
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
        let kind = |s| match s {
            SplitEdgeSnap::Point => ObjectSnapKind::Point,
            SplitEdgeSnap::Mid => ObjectSnapKind::Mid,
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
        let pixel: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        for i in 0..2 {
            assert!((aim[i] - pixel[i]).abs() < 1e-7);
        }
        let setting = &row["value"]["mesh_snap_setting"];
        assert_eq!(setting["before"], setting["restored"]);
        assert_eq!(setting["requested"], fixture.snap_to_meshes.unwrap());
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_options(
                &doc,
                serde_json::from_value(frame["click_client"].clone()).unwrap(),
                12.,
                project,
                ObjectSnapOptions {
                    modes,
                    mesh_edges: fixture.snap_to_meshes.unwrap(),
                },
            )
            .unwrap();
        if id.ends_with("-disabled")
            || matches!(
                id.as_str(),
                "quad-no-diagonal-enabled" | "quad-mid-hover-enabled" | "mid-one-shot"
            )
        {
            // These controls establish mesh admission, not absence of other
            // scene snaps: the receiving box can itself supply a curve Mid.
            assert!(
                snap.is_none_or(|s| s.object_id() != objects[1]),
                "{id}: unexpected mesh capture {snap:?}"
            );
            let mut mesh_only = Document::default();
            mesh_only
                .add_geometry(
                    fixture.base.sources[1]
                        .geometry(Tolerance::DEFAULT)
                        .unwrap(),
                )
                .unwrap();
            assert!(
                ObjectSnapCache::default()
                    .nearest_projected_with_options(
                        &mesh_only,
                        serde_json::from_value(frame["click_client"].clone()).unwrap(),
                        12.,
                        calibrated_snap::projection(frame),
                        ObjectSnapOptions {
                            modes,
                            mesh_edges: fixture.snap_to_meshes.unwrap(),
                        },
                    )
                    .unwrap()
                    .is_none(),
                "{id}: isolated mesh must not capture"
            );
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing capture"));
        let mathematical: [f64; 3] =
            serde_json::from_value(reference[id]["point"].clone()).unwrap();
        assert!(
            snap.point()
                .distance_to(Point3::try_from(mathematical).unwrap())
                .unwrap()
                < 1e-9,
            "{id}"
        );
        assert_eq!(snap.object_id(), objects[1], "{id}");
        let expected_kind = if id.contains("mid") && id != "mid-near-hover" {
            ObjectSnapKind::Mid
        } else {
            ObjectSnapKind::Near
        };
        assert_eq!(snap.kind(), expected_kind, "{id}");
        let mut captured = fixture.clone();
        captured.snap_to_meshes = None;
        let SplitEdgeInput::Pick(target) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        target.point = snap.point().to_array();
        target.osnap = SplitEdgeSnap::Point;
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        let mut expected = calibrated_snap::model_history(&row["value"]);
        expected
            .as_object_mut()
            .unwrap()
            .remove("mesh_snap_setting");
        // All seven now compare the complete, unnormalized observed history at
        // the original epsilon, including the five previous Near discrepancies.
        close(&actual, &expected, id);
        if snap.kind() == ObjectSnapKind::Mid {
            mids += 1;
        } else {
            let old_x = screen_reference[id]["point"][0].as_f64().unwrap();
            assert!(
                (snap.point().x() - old_x).abs() > 1e-9,
                "{id}: mesh is not curve Near"
            );
            near += 1;
        }
        captures += 1;
    }
    assert_eq!((captures, misses), (7, 9));
    assert_eq!((mids, near), (2, 5));
}
