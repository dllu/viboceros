use viboceros_geometry::{AffineTransform3, Brep, Frame3, Point3, Polyline3, Tolerance, Vector3};
use viboceros_io::{
    ThreeDmColorSource, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel, ThreeDmObject,
    read_3dm_file, write_3dm_file,
};
fn extrusion() -> Brep {
    let points = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [0., 0., 0.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let profile = Polyline3::try_new(points, Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_planar_profile_extrusion(&profile, frame, 2., true, Tolerance::DEFAULT).unwrap()
}
#[test]
fn extrusion_3dm_round_trip_preserves_native_type_identity_attributes_groups_and_geometry() {
    let source = extrusion();
    let mut object = ThreeDmObject::new(ThreeDmGeometry::Brep(source.clone()), 0);
    object.name = Some("Native extrusion".into());
    object.group_indices = vec![0];
    object.object_color = [17, 43, 91];
    object.color_source = ThreeDmColorSource::Object;
    object
        .user_text
        .insert("design".into(), "extruded profile".into());
    object
        .geometry_user_text
        .insert("old".into(), "removed after import".into());
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Profiles".into(),
            color: [2, 3, 4],
            visible: true,
            locked: false,
        }],
        vec![ThreeDmGroup {
            name: "Assembly".into(),
        }],
        vec![object.clone()],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extrusion.3dm");
    let report = write_3dm_file(&path, &model).unwrap();
    assert_eq!(report.written_object_count, 1);
    assert_eq!(report.adapted_curve_count, 0);
    let restored = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.unsupported_object_count(), 0);
    assert_eq!(restored.objects.len(), 1);
    let output = &restored.objects[0];
    assert_eq!(output.id, object.id);
    assert_eq!(output.name, object.name);
    assert_eq!(output.group_indices, object.group_indices);
    assert_eq!(output.user_text, object.user_text);
    assert_eq!(output.object_color, object.object_color);
    assert_eq!(output.color_source, object.color_source);
    let ThreeDmGeometry::Brep(brep) = &output.geometry else {
        panic!("extrusion B-rep view missing")
    };
    assert!(brep.is_extrusion());
    assert_eq!(brep.explicit_brep(), source.explicit_brep());
    // Save the imported extrusion again; its native representation must survive.
    let path2 = directory.path().join("resaved.3dm");
    write_3dm_file(&path2, &restored).unwrap();
    let resaved = read_3dm_file(&path2, Tolerance::DEFAULT).unwrap();
    assert_eq!(resaved.objects[0], *output);
}
#[test]
fn transformed_and_edited_extrusions_export_their_current_geometry() {
    let source = extrusion();
    let transformed = source
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(10., 20., 30.).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    let flipped = transformed.reversed();
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![
            ThreeDmObject::new(ThreeDmGeometry::Brep(transformed.clone()), 0),
            ThreeDmObject::new(ThreeDmGeometry::Brep(flipped.clone()), 0),
        ],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("edited.3dm");
    write_3dm_file(&path, &model).unwrap();
    let restored = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    for (i, expected) in [&transformed, &flipped].into_iter().enumerate() {
        let ThreeDmGeometry::Brep(actual) = &restored.objects[i].geometry else {
            panic!("missing extrusion or B-rep")
        };
        assert_eq!(actual.is_extrusion(), expected.is_extrusion());
        assert_eq!(actual.explicit_brep(), expected.explicit_brep());
    }
}

#[test]
fn imported_extrusion_archives_do_not_resurrect_removed_geometry_user_text() {
    let mut object = ThreeDmObject::new(ThreeDmGeometry::Brep(extrusion()), 0);
    object
        .geometry_user_text
        .insert("old".into(), "delete this".into());
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![object],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("text.3dm");
    write_3dm_file(&path, &model).unwrap();
    let mut imported = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects[0].geometry_user_text.len(), 1);
    imported.objects[0].geometry_user_text.clear();
    let path2 = directory.path().join("cleared.3dm");
    write_3dm_file(&path2, &imported).unwrap();
    let restored = read_3dm_file(&path2, Tolerance::DEFAULT).unwrap();
    assert!(restored.objects[0].geometry_user_text.is_empty());
}

#[test]
fn native_rhino_extrusion_files_import_and_resave_with_their_representation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/extrusions");
    for name in [
        "rectangle",
        "rectangle_open",
        "rectangle_negative",
        "circle",
        "circle_open",
        "open_kink",
        "arc",
        "rectangle_miter",
        "circle_shear",
    ] {
        check_native(&root, &format!("tosubd_extrusion_{name}_00"));
    }
    for id in ["tosubd_pipe_000", "tosubd_pipe_100"] {
        check_native(&root, id);
    }
}
fn check_native(root: &std::path::Path, id: &str) {
    let model = read_3dm_file(root.join(format!("{id}.3dm")), Tolerance::DEFAULT)
        .unwrap_or_else(|e| panic!("{id}: {e}"));
    assert_eq!(model.unsupported_object_count(), 0, "{id}");
    assert_eq!(model.objects.len(), 1, "{id}");
    let ThreeDmGeometry::Brep(source) = &model.objects[0].geometry else {
        panic!("{id}: native extrusion lost")
    };
    assert!(source.is_extrusion(), "{id}");
    let observed: serde_json::Value = serde_json::from_str(if id.starts_with("tosubd_pipe_") {
        include_str!("../../../tools/rhino_oracle/observations/tosubd_pipes.json")
    } else {
        include_str!("../../../tools/rhino_oracle/observations/tosubd_extrusions.json")
    })
    .unwrap();
    let target = &observed["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()["value"]["before"][0]["brep"];
    assert_eq!(source.faces().len(), target.as_array().unwrap().len());
    let mut maximum_surface_error = 0_f64;
    let mut maximum_control_error = 0_f64;
    let mut maximum_weight_error = 0_f64;
    for (face_index, (actual, native)) in source
        .faces()
        .iter()
        .zip(target.as_array().unwrap())
        .enumerate()
    {
        assert_eq!(actual.is_reversed(), native["reversed"].as_bool().unwrap());
        assert_eq!(
            actual.loops().len(),
            native["loops"].as_u64().unwrap() as usize
        );
        let expected = native_surface(&native["surface"]);
        if expected.control_points().iter().any(|p| p.weight() != 1.) {
            assert_eq!(
                [actual.surface().degree_u(), actual.surface().degree_v()],
                [expected.degree_u(), expected.degree_v()],
                "{id}"
            );
            assert_eq!(
                [
                    actual.surface().control_point_count_u(),
                    actual.surface().control_point_count_v()
                ],
                [
                    expected.control_point_count_u(),
                    expected.control_point_count_v()
                ],
                "{id}"
            );
            assert_eq!(actual.surface().knots_u(), expected.knots_u(), "{id}");
            assert_eq!(actual.surface().knots_v(), expected.knots_v(), "{id}");
            for (a, b) in actual
                .surface()
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                let error = a.point().distance_to(b.point()).unwrap();
                let weight_error = (a.weight() - b.weight()).abs();
                assert!(
                    error <= 1e-6 && weight_error <= 1e-12,
                    "{id}: rational control net differs"
                );
                maximum_control_error = maximum_control_error.max(error);
                maximum_weight_error = maximum_weight_error.max(weight_error);
            }
        }

        // Cap support-plane rectangles may expand when their extrusion is
        // archived. Compare shared parameter stations; visible caps are trimmed.
        let a = [actual.surface().domain_u(), actual.surface().domain_v()];
        let b = [expected.domain_u(), expected.domain_v()];
        let common: [[f64; 2]; 2] = std::array::from_fn(|axis| {
            [
                a[axis].start().max(*b[axis].start()),
                a[axis].end().min(*b[axis].end()),
            ]
        });
        assert!(
            common.iter().all(|d| d[0] < d[1]),
            "{id}: face parameter domains do not overlap"
        );
        for i in 0..=8 {
            for j in 0..=4 {
                let u = common[0][0] + (common[0][1] - common[0][0]) * i as f64 / 8.;
                let v = common[1][0] + (common[1][1] - common[1][0]) * j as f64 / 4.;
                let error = actual
                    .surface()
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected.evaluate(u, v).unwrap())
                    .unwrap();
                assert!(
                    error <= 1e-6,
                    "{id}: face {face_index} common uv {u},{v} differs by {error}"
                );
                maximum_surface_error = maximum_surface_error.max(error);
            }
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resaved.3dm");
    write_3dm_file(&destination, &model).unwrap_or_else(|e| panic!("{id}: {e}"));
    let saved = read_3dm_file(&destination, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::Brep(actual) = &saved.objects[0].geometry else {
        panic!("{id}: resaved extrusion lost")
    };
    assert!(actual.is_extrusion());
    assert_eq!(actual.explicit_brep(), source.explicit_brep(), "{id}");
    println!(
        "\nEXTRUSION_NATIVE_FILE {}",
        serde_json::json!({"id":id,"vertices":source.vertices().len(),"edges":source.edges().len(),"faces":source.faces().len(),"solid":source.is_solid(),"native_type_retained":true,"canonical_view_unchanged":true,"identity_retained":saved.objects[0].id==model.objects[0].id,"native_surface_stations_and_orientation":true,"maximum_surface_sample_error":maximum_surface_error,"rational_control_nets_retained":true,"maximum_rational_control_error":maximum_control_error,"maximum_rational_weight_error":maximum_weight_error})
    );
}
#[test]
fn invalid_native_profile_frame_fixture_is_reported_as_unsupported() {
    // The public Rhino diagnostic confirms an invalid B-rep form for this
    // misplaced inner profile. Correctly constructed pipes are covered above.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/extrusions/tosubd_extrusion_tube_00.3dm");
    let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    assert_eq!(model.unsupported_object_count(), 1);
    assert!(model.objects.is_empty());
    println!(
        "\nEXTRUSION_NATIVE_REJECTED {}",
        serde_json::json!({"id":"tosubd_extrusion_tube_00","native_invalid_canonical_view":true,"unsupported_object_count":1})
    );
}

fn native_surface(v: &serde_json::Value) -> viboceros_geometry::NurbsSurface {
    use viboceros_geometry::{NurbsSurface, WeightedPoint3};
    let coordinates = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(
                Point3::try_new(
                    c["point"][0].as_f64().unwrap(),
                    c["point"][1].as_f64().unwrap(),
                    c["point"][2].as_f64().unwrap(),
                )
                .unwrap(),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let knots = |name: &str| {
        v[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    };
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        coordinates,
        knots("knots_u"),
        knots("knots_v"),
    )
    .unwrap()
}

#[path = "extrusion/rational_weights.rs"]
mod rational_weights;
