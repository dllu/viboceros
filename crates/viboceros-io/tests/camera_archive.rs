//! Native 3DM cameras whose cached frames cannot be recovered from raw vectors.
use serde_json::Value;
use viboceros_geometry::{LengthUnitSystem, Tolerance, Vector3};
use viboceros_io::{
    ThreeDmNamedView, read_3dm_file, read_3dm_named_views_file_in_units, write_3dm_file,
};

fn corpus() -> (std::path::PathBuf, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let native = serde_json::from_slice(
        &std::fs::read(root.join("observations/camera_archive.json")).unwrap(),
    )
    .unwrap();
    (root, native)
}
fn close(a: [f64; 3], b: [f64; 3]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 2e-12, "{a} vs {b}");
    }
}
fn native_frame(view: &ThreeDmNamedView, value: &Value) {
    assert!(view.camera_axes.is_some());
    for (axis, native) in view
        .camera_frame()
        .unwrap()
        .axes()
        .into_iter()
        .zip(value["frame"].as_array().unwrap())
    {
        close(
            axis.as_vector().to_array(),
            serde_json::from_value(native.clone()).unwrap(),
        );
    }
    assert_eq!(
        view.camera_direction.to_array(),
        serde_json::from_value::<[f64; 3]>(value["direction"].clone()).unwrap()
    );
    assert_eq!(
        view.camera_up.to_array(),
        serde_json::from_value::<[f64; 3]>(value["up"].clone()).unwrap()
    );
    close(
        view.camera_location.to_array(),
        serde_json::from_value(value["location"].clone()).unwrap(),
    );
}

#[test]
fn cached_camera_frames_survive_native_files_round_trips_and_unit_conversion() {
    let (root, native) = corpus();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("camera.3dm");
    let rows = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), 18);
    for row in rows {
        let value = &row["value"];
        let mut model = read_3dm_file(
            root.join(value["file"].as_str().unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(model.named_views.len(), 1);
        native_frame(&model.named_views[0], &value["archive"]);
        model.units = LengthUnitSystem::Millimeters;
        for _ in 0..3 {
            write_3dm_file(&path, &model).unwrap();
            let decoded = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
            let a = &decoded.named_views[0];
            let b = &model.named_views[0];
            native_frame(a, &value["archive"]);
            assert_eq!(a.projection, b.projection);
            assert_eq!(a.target, b.target);
            assert_eq!(a.frustum, b.frustum);
            assert_eq!(a.screen_port, b.screen_port);
            model = decoded;
        }
        let scaled = read_3dm_named_views_file_in_units(&path, &LengthUnitSystem::Meters).unwrap();
        let source = &model.named_views[0];
        close(
            scaled[0].camera_location.to_array(),
            source.camera_location.to_array().map(|x| x * 0.001),
        );
        for (a, b) in scaled[0]
            .camera_frame()
            .unwrap()
            .axes()
            .into_iter()
            .zip(source.camera_frame().unwrap().axes())
        {
            close(a.as_vector().to_array(), b.as_vector().to_array());
        }
    }
}

#[test]
fn invalid_cached_frames_reject_writes_before_touching_the_destination() {
    let (root, native) = corpus();
    let row = &native["results"][0]["value"];
    let mut model =
        read_3dm_file(root.join(row["file"].as_str().unwrap()), Tolerance::DEFAULT).unwrap();
    let axes = model.named_views[0].camera_axes.unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("preserve.3dm");
    std::fs::write(&path, b"preserve").unwrap();
    for invalid in [
        [axes[0], axes[1], axes[2].opposite()],
        [axes[0], axes[0], axes[2]],
    ] {
        model.named_views[0].camera_axes = Some(invalid);
        assert!(model.named_views[0].camera_frame().is_err());
        assert!(write_3dm_file(&path, &model).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve");
    }
    model.named_views[0].camera_axes = Some(axes);
    for direction in [
        Vector3::try_from([0.; 3]).unwrap(),
        model.named_views[0].camera_up,
    ] {
        model.named_views[0].camera_direction = direction;
        assert!(model.named_views[0].camera_frame().is_err());
        assert!(write_3dm_file(&path, &model).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve");
    }
}
