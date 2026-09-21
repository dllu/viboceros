use super::*;
use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};

#[test]
fn elliptic_snaps_replay_full_history_with_explicit_gauge_and_short_arc_differences() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/elliptic_center_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/elliptic_center_snaps.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 62);
    assert_eq!(observed["results"].as_array().unwrap().len(), 62);
    let (mut matches, mut misses, mut gauges, mut short_arcs) = (0, 0, 0, 0);
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
        assert_eq!(value["succeeded"], true, "{id}");
        assert_eq!(value["history_tested"], true, "{id}");
        let [frame] = value["pick_frames"].as_array().unwrap().as_slice() else {
            panic!()
        };
        let project = calibrated_snap::projection(frame);
        let pixel = project(Point3::try_from(pick.aim.unwrap()).unwrap()).unwrap();
        let expected: [f64; 2] = serde_json::from_value(frame["aim_client"].clone()).unwrap();
        for i in 0..2 {
            assert!((pixel[i] - expected[i]).abs() < 1e-7, "{id}");
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
            .map(|source| {
                document
                    .add_geometry(source.geometry(Tolerance::DEFAULT).unwrap())
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
        let observed_x = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
            .as_f64()
            .unwrap();
        if [
            "parabola-",
            "hyperbola-",
            "altered-span-",
            "nonplanar-",
            "near-conic-",
        ]
        .iter()
        .any(|prefix| id.starts_with(prefix))
        {
            assert!(snap.is_none(), "{id}: {snap:?}");
            assert!((observed_x - 4.).abs() > 2.);
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing native ellipse Center"));
        assert_eq!(snap.kind(), ObjectSnapKind::Center);
        assert_eq!(snap.object_id(), objects[1]);
        assert!(
            snap.point()
                .distance_to(Point3::try_from(pick.point).unwrap())
                .unwrap()
                < 1e-9,
            "{id}: {snap:?}"
        );
        if ["positive-gauge-", "negative-gauge-", "large-gauge-"]
            .iter()
            .any(|prefix| id.starts_with(prefix))
        {
            assert!((observed_x - snap.point().x()).abs() > 2.);
            gauges += 1;
            continue;
        }
        if id.starts_with("short-arc-0.01-") || id.starts_with("short-arc-0.001-") {
            assert!((observed_x - snap.point().x()).abs() > 1e-7);
            short_arcs += 1;
            continue;
        }
        let mut captured = fixture.clone();
        let SplitEdgeInput::Pick(input) = &mut captured.inputs.as_mut().unwrap()[0] else {
            panic!()
        };
        input.point = snap.point().to_array();
        let (actual, _) = run_split(&captured, Tolerance::DEFAULT).unwrap();
        close(&actual, &calibrated_snap::model_history(value), id);
        matches += 1;
    }
    assert_eq!((matches, misses, gauges, short_arcs), (42, 10, 6, 4));
}
