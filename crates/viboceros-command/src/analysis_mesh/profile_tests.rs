use super::*;
use viboceros_io::{
    ThreeDmModel, read_3dm_file_with_blocks_model_tolerance as read, write_3dm_file,
};

fn compare(a: &ThreeDmModel, b: &ThreeDmModel) {
    assert_eq!(a.analysis_nurbs_density, b.analysis_nurbs_density);
    assert_eq!(a.analysis_subd_density, b.analysis_subd_density);
    assert_eq!(a.analysis_simple_planes, b.analysis_simple_planes);
    assert_eq!(a.analysis_mesh_parameters, b.analysis_mesh_parameters);
}

fn add_plane(doc: &mut Document) -> viboceros_document::ObjectId {
    use viboceros_geometry::{NurbsSurface, Point3};
    let surface = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    id
}

#[test]
fn analysis_profile_raw_retention_and_document_migration_match_native_load_save() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let temp = tempfile::tempdir().unwrap();
    let saved = temp.path().join("saved.3dm");
    let native_path = temp.path().join("native.3dm");
    for family in ["analysis_profile", "analysis_profile_identity"] {
        let native: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("observations").join(format!("{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in native["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let input = root
                .join("fixtures/analysis_profiles")
                .join(format!("{id}.3dm"));
            let raw = read(&input).unwrap();
            write_3dm_file(&saved, &raw).unwrap();
            compare(&read(&saved).unwrap(), &raw);
            let loaded = viboceros_io::read_3dm_file_for_document(&input).unwrap();
            let (doc, _, _, _, _) =
                open_3dm_with_views_and_cplanes(input.to_str().unwrap()).unwrap();
            assert_eq!(doc.analysis_nurbs_density(), loaded.analysis_nurbs_density);
            assert_eq!(doc.analysis_simple_planes(), loaded.analysis_simple_planes);
            assert_eq!(doc.analysis_subd_density(), loaded.analysis_subd_density);
            assert_eq!(
                doc.analysis_mesh_parameters(),
                loaded.analysis_mesh_parameters.as_ref()
            );
            save_3dm_with_named_views(&doc, saved.to_str().unwrap(), &[]).unwrap();
            let exported = read(&saved).unwrap();
            compare(&exported, &loaded);
            for variant in row["value"]["variants"].as_array().unwrap() {
                let hex = variant["archive_hex"].as_str().unwrap();
                let bytes = hex
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                    .collect::<Vec<_>>();
                std::fs::write(&native_path, bytes).unwrap();
                let expected = read(&native_path).unwrap();
                assert_eq!(
                    loaded.analysis_nurbs_density.value(),
                    variant["after"]["RelativeTolerance"].as_f64().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    loaded.analysis_simple_planes,
                    variant["after"]["SimplePlanes"].as_bool().unwrap(),
                    "{id}"
                );
                compare(&loaded, &expected);
            }
        }
    }
}

#[test]
fn analysis_profile_edits_preserve_detailed_controls_and_defaults_clear_them() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/analysis_profiles");
    let (mut doc, _, _, _, _) =
        open_3dm_with_views_and_cplanes(root.join("zero-tolerance.3dm").to_str().unwrap()).unwrap();
    let retained = doc.analysis_mesh_parameters().unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    add_plane(&mut doc);
    registry
        .execute(
            &mut doc,
            "DraftAngleAnalysis NurbsDensity=0.5 SimplePlanes=Yes SubDLevel=2 SubDAdaptive=No",
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &retained,
        doc.analysis_mesh_parameters().unwrap()
    ));
    let settings = doc.analysis_mesh_settings();
    registry
        .execute(
            &mut doc,
            &format!("Import3dm {}", root.join("zero.3dm").display()),
        )
        .unwrap();
    assert_eq!(doc.analysis_mesh_settings(), settings);
    assert!(Arc::ptr_eq(
        &retained,
        doc.analysis_mesh_parameters().unwrap()
    ));
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("edited.3dm");
    save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
    let edited = read(&path).unwrap();
    assert!(edited.analysis_mesh_parameters.is_some());
    assert_eq!(edited.analysis_nurbs_density.value(), 0.5);
    assert!(edited.analysis_simple_planes);
    assert_eq!(
        edited.analysis_subd_density,
        viboceros_geometry::SubDMeshDensity::new(2, false).unwrap()
    );
    let mut reverted = edited;
    reverted.analysis_nurbs_density = viboceros_geometry::NurbsMeshDensity::new(0.).unwrap();
    reverted.analysis_simple_planes = false;
    reverted.analysis_subd_density = Default::default();
    write_3dm_file(&path, &reverted).unwrap();
    assert_eq!(
        read(&path).unwrap().analysis_mesh_parameters.as_ref(),
        Some(&retained)
    );
    registry
        .execute(&mut doc, "DraftAngleAnalysis MeshDefaults=Yes")
        .unwrap();
    assert_eq!(doc.analysis_mesh_settings(), Default::default());
    assert!(doc.analysis_mesh_parameters().is_none());
    save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
    assert!(read(&path).unwrap().analysis_mesh_parameters.is_none());
}

#[test]
fn analysis_profile_invalid_retained_archive_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let mut model = ThreeDmModel::new(vec![], vec![], vec![]);
    for bytes in [vec![1, 2, 3], vec![0; 65537]] {
        model.analysis_mesh_parameters = Some(Arc::from(bytes));
        assert!(write_3dm_file(temp.path().join("invalid.3dm"), &model).is_err());
    }
}

#[test]
fn analysis_profile_simple_planes_refresh_shared_meshes_without_model_edits() {
    use viboceros_geometry::{NurbsSurface, Point3};
    let mut doc = Document::default();
    let surface = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    let old = registry.draft_angle_analysis_view(&doc).unwrap();
    let grid = old.mesh(doc.object(id).unwrap(), &doc).unwrap();
    let source = doc.object(id).unwrap().geometry_snapshot().clone();
    registry
        .execute(&mut doc, "DraftAngleAnalysis SimplePlanes=Yes")
        .unwrap();
    let simple = old.mesh(doc.object(id).unwrap(), &doc).unwrap();
    assert_eq!(grid.mesh.faces().len(), 16);
    assert_eq!(simple.mesh.faces().len(), 1);
    assert!(source.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "CurvatureAnalysis").unwrap();
    assert!(Arc::ptr_eq(
        &simple,
        &registry.curvature_analysis_view(&doc).unwrap().meshes[&id]
    ));
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ExtractAnalysisMesh").unwrap();
    let mesh = doc
        .objects()
        .find_map(|o| {
            if let Geometry::Mesh(m) = o.geometry() {
                Some(m)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(mesh.faces().len(), 1);
    assert_eq!(mesh.vertex_colors().unwrap().len(), 4);
    assert_eq!(mesh.vertex_normals().unwrap().len(), 4);
}
